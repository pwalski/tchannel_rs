use crate::channel::TResult;
use crate::fragmentation::FragmentationStatus::{CompleteAtTheEnd, Incomplete};
use crate::frames::headers::TransportHeaderKey::CallerName;
use crate::frames::headers::{ArgSchemeValue, TransportHeaderKey};
use crate::frames::payloads::Codec;
use crate::frames::payloads::{
    CallArgs, CallContinue, CallRequestFields, CallResponseFields, CallWithFieldsEncoded,
    ChecksumType, Flags, TraceFlags, Tracing, ARG_LEN_LEN,
};
use crate::frames::{TFrame, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH};
use crate::messages::args::{MessageArgs, ResponseCode};
use bytes::Buf;
use bytes::Bytes;
use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub(crate) struct ResponseFragmenter {
    fragmenter: Fragmenter,
    response_code: ResponseCode,
}

impl ResponseFragmenter {
    pub fn new(
        service_name: impl AsRef<str>,
        response_code: ResponseCode,
        args: MessageArgs,
    ) -> ResponseFragmenter {
        ResponseFragmenter {
            fragmenter: Fragmenter::new(service_name.as_ref().to_string(), args),
            response_code,
        }
    }

    pub fn create_frames(self) -> TResult<TFrameStream> {
        let fields = self.create_response_fields();
        self.fragmenter
            .create_frames(fields, Type::CallResponse, Type::CallResponseContinue)
    }

    fn create_response_fields(&self) -> CallResponseFields {
        let tracing = create_tracing();
        let headers = self.create_headers(); //TODO get rid of clones
        CallResponseFields::new(self.response_code, tracing, headers)
        //TODO configurable TTL
    }

    fn create_headers(&self) -> HashMap<String, String> {
        self.fragmenter.create_headers()
    }
}

#[derive(Debug)]
pub struct RequestFragmenter {
    fragmenter: Fragmenter,
}

impl RequestFragmenter {
    pub(crate) fn new(service_name: String, args: MessageArgs) -> RequestFragmenter {
        let fragmenter = Fragmenter::new(service_name, args);
        RequestFragmenter { fragmenter }
    }

    pub fn create_frames(self) -> TResult<TFrameStream> {
        let fields = self.create_fields();
        self.fragmenter
            .create_frames(fields, Type::CallRequest, Type::CallRequestContinue)
    }

    fn create_fields(&self) -> CallRequestFields {
        let tracing = create_tracing();
        let headers = self.create_headers(); //TODO get rid of clones
        CallRequestFields::new(
            60_000,
            tracing,
            self.fragmenter.service_name.clone(),
            headers,
        )
        //TODO configurable TTL
    }

    fn create_headers(&self) -> HashMap<String, String> {
        self.fragmenter.create_headers()
        //TODO add claim add start/finish headers
        //TODO add failure domain header
        //TODO add retry flag header
        //TODO add speculative execution header
        //TODO add short key header
        //TODO add routing delegate header
    }
}

#[derive(Debug)]
struct Fragmenter {
    service_name: String,
    arg_scheme: ArgSchemeValue,
    args_fragmenter: ArgsFragmenter,
}

impl Fragmenter {
    pub fn new(service_name: String, args: MessageArgs) -> Fragmenter {
        Fragmenter {
            service_name,
            arg_scheme: args.arg_scheme,
            args_fragmenter: ArgsFragmenter::new(args.args),
        }
    }

    pub fn create_frames<FIELDS: Codec>(
        mut self,
        fields: FIELDS,
        first_type: Type,
        continuation_type: Type,
    ) -> TResult<TFrameStream> {
        let fields_bytes = fields.encode_bytes()?;
        let payload_limit = calculate_payload_limit(fields_bytes.len());
        let frame_args = self.next_frame_args(payload_limit);
        let flag = self.current_frame_flag();

        let mut frames = Vec::new();
        let call_request = CallWithFieldsEncoded::new(flag, fields_bytes, frame_args);
        trace!("Creating call request {:?}", call_request);
        frames.push(TFrame::new(first_type, call_request.encode_bytes()?));

        while !self.args_fragmenter.is_empty() {
            trace!("Creating call continuation");
            let payload_limit = calculate_payload_limit(0);
            let frame_args = self.next_frame_args(payload_limit);
            let flag = self.current_frame_flag();
            let call_continue = CallContinue::new(flag, frame_args);
            frames.push(TFrame::new(
                continuation_type,
                call_continue.encode_bytes()?,
            ))
        }
        debug!("Message fragmented into {} frames", frames.len());
        Ok(Box::pin(futures::stream::iter(frames)))
    }

    fn next_frame_args(&mut self, payload_limit: usize) -> CallArgs {
        let checksum_type = ChecksumType::None; //TODO hardcoded. Make it configurable
        let frame_args = self.next_frame_args_vec(payload_limit, checksum_type);
        let checksum = calculate_checksum(&frame_args, checksum_type);
        CallArgs::new(checksum_type, checksum, frame_args)
    }

    fn next_frame_args_vec(
        &mut self,
        payload_limit: usize,
        checksum_type: ChecksumType,
    ) -> VecDeque<Option<Bytes>> {
        let remaining_limit = payload_limit as i32 - checksum_len(checksum_type) as i32;
        self.args_fragmenter.next_frame_args_vec(remaining_limit)
    }

    fn current_frame_flag(&self) -> Flags {
        if self.args_fragmenter.is_empty() {
            Flags::NONE
        } else {
            Flags::MORE_FRAGMENTS_FOLLOW
        }
    }

    pub fn create_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            TransportHeaderKey::ArgScheme.to_string(),
            self.arg_scheme.to_string(),
        );
        headers.insert(CallerName.to_string(), self.service_name.clone());
        //TODO add failure domain
        headers
    }
}

#[derive(Debug)]
struct ArgsFragmenter {
    args: VecDeque<Bytes>,
}

impl ArgsFragmenter {
    pub fn new(args: Vec<Bytes>) -> Self {
        Self {
            args: VecDeque::from(args),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    pub fn next_frame_args_vec(&mut self, mut payload_limit: i32) -> VecDeque<Option<Bytes>> {
        let mut frame_args = VecDeque::with_capacity(3);
        while let Some(mut arg) = self.args.pop_front() {
            payload_limit -= ARG_LEN_LEN as i32;
            let (status, frame_arg_opt) = Self::next_arg_fragment(&mut arg, payload_limit);
            if let Some(frame_arg) = frame_arg_opt {
                match frame_arg.len() {
                    0 => frame_args.push_back(None),
                    len => {
                        payload_limit -= len as i32;
                        frame_args.push_back(Some(frame_arg));
                    }
                }
            }
            if status == Incomplete {
                self.args.push_front(arg);
                break;
            } else if status == CompleteAtTheEnd {
                self.args.push_front(Bytes::new());
                break;
            }
        }
        frame_args
    }

    fn next_arg_fragment(
        arg: &mut Bytes,
        payload_limit: i32,
    ) -> (FragmentationStatus, Option<Bytes>) {
        let src_remaining = arg.remaining() as i32;
        if src_remaining == 0 {
            (FragmentationStatus::Complete, Some(Bytes::new()))
        } else if payload_limit < 0 {
            (FragmentationStatus::Incomplete, None)
        } else if src_remaining > payload_limit {
            let fragment = arg.split_to(payload_limit as usize);
            (FragmentationStatus::Incomplete, Some(fragment))
        } else {
            let arg_bytes = arg.split_to(arg.len());
            if src_remaining == payload_limit {
                (FragmentationStatus::CompleteAtTheEnd, Some(arg_bytes))
            } else {
                (FragmentationStatus::Complete, Some(arg_bytes))
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum FragmentationStatus {
    //Completely stored in frame
    Complete,
    //Completely stored in frame with maxed frame capacity
    CompleteAtTheEnd,
    //Partially stored in frame
    Incomplete,
}

fn calculate_payload_limit(fields_len: usize) -> usize {
    //64KiB max frame size - header size -1 (flag) - serialized fields size
    FRAME_MAX_LENGTH as usize - FRAME_HEADER_LENGTH as usize - 1 - fields_len
}

fn create_tracing() -> Tracing {
    Tracing::new(0, 0, 0, TraceFlags::NONE)
}

fn checksum_len(checksum_type: ChecksumType) -> usize {
    match checksum_type {
        ChecksumType::None => 1, // checksum_type
        _ => 5,                  // checksum_type + checksum
    }
}

fn calculate_checksum(_args: &VecDeque<Option<Bytes>>, csum_type: ChecksumType) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::payloads::CallContinue;
    use crate::frames::payloads::CallRequest;
    use crate::frames::TFrame;
    use futures::StreamExt;
    use tokio_test::*;

    const SERVICE_NAME: &str = "test_service";
    const ARG_SCHEME: ArgSchemeValue = ArgSchemeValue::Json;

    #[test]
    fn single_frame() {
        // GIVEN
        let args_bytes = [Bytes::from("a"), Bytes::from("b"), Bytes::from("c")].to_vec();

        // WHEN
        let args = MessageArgs::new(ARG_SCHEME, args_bytes);
        let fragmenter = RequestFragmenter::new(SERVICE_NAME.to_string(), args);
        let frames_res = fragmenter.create_frames();

        // THEN
        assert!(frames_res.is_ok(), "{}", frames_res.err().unwrap());
        let mut frames: Vec<TFrame> = block_on(frames_res.unwrap().collect());
        assert_eq!(frames.len(), 1);
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequest, *frame.frame_type());
        let call_req_res = CallRequest::decode(frame.payload_mut());
        assert!(call_req_res.is_ok(), "{}", call_req_res.err().unwrap());
        let mut call_req = call_req_res.unwrap();
        assert_eq!(Flags::NONE, *call_req.flags());
        assert_eq!(SERVICE_NAME.to_string(), *call_req.fields().service());
        let headers = call_req.fields().headers();
        assert_eq!(
            ARG_SCHEME.to_string(),
            *headers
                .get(&TransportHeaderKey::ArgScheme.to_string())
                .unwrap()
        );
        assert_eq!(ChecksumType::None, *call_req.args().checksum_type());
        let args = call_req.args_mut().args_mut();
        assert_eq!(Bytes::from("a"), args.get(0).unwrap().as_ref().unwrap());
        assert_eq!(Bytes::from("b"), args.get(1).unwrap().as_ref().unwrap());
        assert_eq!(Bytes::from("c"), args.get(2).unwrap().as_ref().unwrap());
    }

    #[test]
    fn multiple_frames() {
        // GIVEN
        // arg1 will take whole 1st frame and part of 2nd frame
        let arg1 = Bytes::from(vec![b'a'; (u16::MAX) as usize]);
        // arg2 will take part of 2nd frame
        let arg2 = Bytes::from(vec![b'b'; (u16::MAX / 2) as usize]);
        // arg3 will take remaining space of 2nd frame and part of 3rd frame
        let arg3 = Bytes::from(vec![b'c'; (u16::MAX) as usize]);

        // WHEN
        let args = MessageArgs::new(
            ARG_SCHEME,
            [arg1.clone(), arg2.clone(), arg3.clone()].into(),
        );
        let fragmenter = RequestFragmenter::new(SERVICE_NAME.to_string(), args);
        let frames_res = fragmenter.create_frames();

        // THEN
        assert!(frames_res.is_ok(), "{}", frames_res.err().unwrap());
        let mut frames: Vec<TFrame> = block_on(frames_res.unwrap().collect());
        assert_eq!(frames.len(), 3);
        frames.reverse();

        // frame 1
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequest, *frame.frame_type());
        let mut call_req = CallRequest::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::MORE_FRAGMENTS_FOLLOW, *call_req.flags());
        let args = call_req.args_mut().args_mut();
        assert_eq!(1, args.len());
        let arg1_1 = args.pop_front().unwrap().unwrap();

        // frame 2
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequestContinue, *frame.frame_type());
        let mut call_req = CallContinue::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::MORE_FRAGMENTS_FOLLOW, *call_req.flags());
        let args = call_req.args_mut().args_mut();
        assert_eq!(3, args.len());
        let arg1_2 = args.pop_front().unwrap().unwrap();
        let arg2_1 = args.pop_front().unwrap().unwrap();
        let arg3_1 = args.pop_front().unwrap().unwrap();

        // frame 3
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequestContinue, *frame.frame_type());
        let mut call_req = CallContinue::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::NONE, *call_req.flags());
        let args = call_req.args_mut().args_mut();
        assert_eq!(1, args.len());
        let arg3_2 = args.pop_front().unwrap().unwrap();

        // verify args
        let len = arg1_1.len() + arg1_2.len();
        assert_eq!(arg1, arg1_1.chain(arg1_2).copy_to_bytes(len));
        assert_eq!(arg2, arg2_1);
        let len = arg3_1.len() + arg3_2.len();
        assert_eq!(arg3, arg3_1.chain(arg3_2).copy_to_bytes(len));
    }
}
