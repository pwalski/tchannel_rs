use crate::errors::TChannelError;
use crate::fragmentation::FragmentationStatus::{CompleteAtTheEnd, Incomplete};
use crate::frames::headers::TransportHeader::CallerNameKey;
use crate::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::frames::payloads::Codec;
use crate::frames::payloads::{
    CallArgs, CallContinue, CallFieldsEncoded, CallRequestFields, ChecksumType, Flags, TraceFlags,
    Tracing, ARG_LEN_LEN,
};
use crate::frames::{TFrame, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH};
use crate::messages::{Message, Request};
use bytes::Buf;
use bytes::{Bytes, BytesMut};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, new)]
pub struct Fragmenter {
    service_name: String,
    arg_scheme: ArgSchemeValue,
    args: Vec<Bytes>,
}

impl Fragmenter {
    pub fn create_frames(mut self) -> Result<TFrameStream, TChannelError> {
        self.args.reverse();

        let request_fields = self.create_request_fields_bytes()?;
        let payload_limit = calculate_payload_limit(request_fields.len());
        let frame_args = self.next_frame_args(payload_limit);
        let flag = self.current_frame_flag();

        let mut call_frames = Vec::new();
        let call_request = CallFieldsEncoded::new(flag, request_fields, frame_args);
        debug!("Creating call request {:?}", call_request);
        call_frames.push(TFrame::new(Type::CallRequest, call_request.encode_bytes()?));

        while !self.args.is_empty() {
            debug!("Creating call continue");
            let payload_limit = calculate_payload_limit(0);
            let frame_args = self.next_frame_args(payload_limit);
            let flag = self.current_frame_flag();
            let call_continue = CallContinue::new(flag, frame_args);
            call_frames.push(TFrame::new(
                Type::CallRequestContinue,
                call_continue.encode_bytes()?,
            ))
        }
        debug!("Done! {} frames", call_frames.len());
        Ok(Box::pin(futures::stream::iter(call_frames)))
    }

    fn create_request_fields_bytes(&self) -> Result<Bytes, TChannelError> {
        let mut bytes = BytesMut::new();
        self.create_request_fields().encode(&mut bytes)?;
        Ok(Bytes::from(bytes))
    }

    fn create_request_fields(&self) -> CallRequestFields {
        let tracing = create_tracing();
        let headers = self.create_headers(); //TODO get rid of clones
        CallRequestFields::new(60_000, tracing, self.service_name.clone(), headers)
        //TODO configurable TTL
    }

    fn create_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            TransportHeader::ArgSchemeKey.to_string(),
            self.arg_scheme.to_string(),
        );
        headers.insert(CallerNameKey.to_string(), self.service_name.clone());
        headers
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
        let mut frame_args = VecDeque::with_capacity(3);
        let mut remaining_limit = payload_limit - checksum_len(checksum_type);
        while let Some(mut arg) = self.args.pop() {
            remaining_limit -= ARG_LEN_LEN;
            let (status, frame_arg_bytes) = fragment_arg(&mut arg, remaining_limit);
            frame_arg_bytes.map(|frame_arg| match frame_arg.len() {
                0 => frame_args.push_back(None),
                len => {
                    remaining_limit -= len;
                    frame_args.push_back(Some(frame_arg));
                }
            });
            if status == Incomplete {
                self.args.push(arg);
                break;
            } else if status == CompleteAtTheEnd {
                self.args.push(Bytes::new());
                break;
            }
        }
        frame_args
    }

    fn current_frame_flag(&self) -> Flags {
        if self.args.is_empty() {
            Flags::NONE
        } else {
            Flags::MORE_FRAGMENTS_FOLLOW
        }
    }
}

#[derive(PartialEq)]
enum FragmentationStatus {
    //Completely stored in frame
    Complete,
    //Completely stored in frame with maxed frame capacity
    CompleteAtTheEnd,
    //Partially stored in frame
    Incomplete,
}

fn fragment_arg(arg: &mut Bytes, payload_limit: usize) -> (FragmentationStatus, Option<Bytes>) {
    let src_remaining = arg.remaining();
    if src_remaining == 0 {
        (FragmentationStatus::Complete, None)
    } else if payload_limit <= 0 {
        (FragmentationStatus::Incomplete, None)
    } else if src_remaining > payload_limit {
        let fragment = arg.split_to(payload_limit);
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

fn calculate_checksum(args: &VecDeque<Option<Bytes>>, csum_type: ChecksumType) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::payloads::CallContinue;
    use crate::frames::payloads::CallRequest;
    use crate::frames::TFrame;
    use futures::StreamExt;
    use tokio_test::*;

    const service_name: &str = "test_service";
    const arg_scheme: ArgSchemeValue = ArgSchemeValue::Json;

    #[test]
    fn single_frame() {
        // Given
        let args = [Bytes::from("a"), Bytes::from("b"), Bytes::from("c")].to_vec();
        let fragmenter = Fragmenter::new(service_name.to_string(), arg_scheme, args);

        // When
        let frames_res = fragmenter.create_frames();

        // Then
        assert!(!frames_res.is_err(), frames_res.err().unwrap());
        let mut frames: Vec<TFrame> = block_on(frames_res.unwrap().collect());
        assert_eq!(frames.len(), 1);
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequest, *frame.frame_type());
        let mut call_req_res = CallRequest::decode(frame.payload_mut());
        assert!(!call_req_res.is_err(), call_req_res.err().unwrap());
        let mut call_req = call_req_res.unwrap();
        assert_eq!(Flags::NONE, *call_req.flags());
        assert_eq!(service_name.to_string(), *call_req.fields().service());
        let headers = call_req.fields().headers();
        assert_eq!(
            arg_scheme.to_string(),
            *headers
                .get(&TransportHeader::ArgSchemeKey.to_string())
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
        // Given
        // arg1 will take whole 1st frame and part of 2nd frame
        let arg1 = Bytes::from(vec!['a' as u8; (u16::MAX) as usize]);
        // arg2 will take part of 2nd frame
        let arg2 = Bytes::from(vec!['b' as u8; (u16::MAX / 2) as usize]);
        // arg3 will take remaining space of 2nd frame and part of 3rd frame
        let arg3 = Bytes::from(vec!['c' as u8; (u16::MAX) as usize]);
        let fragmenter = Fragmenter::new(
            service_name.to_string(),
            arg_scheme,
            [arg1.clone(), arg2.clone(), arg3.clone()].to_vec(),
        );

        // When
        let frames_res = fragmenter.create_frames();

        // Then
        assert!(!frames_res.is_err(), frames_res.err().unwrap());
        let mut frames: Vec<TFrame> = block_on(frames_res.unwrap().collect());
        assert_eq!(frames.len(), 3);
        frames.reverse();
        // frame 1
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequest, *frame.frame_type());
        let mut call_req = CallRequest::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::MORE_FRAGMENTS_FOLLOW, *call_req.flags());
        let mut args = call_req.args_mut().args_mut();
        assert_eq!(1, args.len());
        let mut arg1_1 = args.pop_front().unwrap().unwrap();
        // frame 2
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequestContinue, *frame.frame_type());
        let mut call_req = CallContinue::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::MORE_FRAGMENTS_FOLLOW, *call_req.flags());
        let args = call_req.args_mut().args_mut();
        assert_eq!(3, args.len());
        let mut arg1_2 = args.pop_front().unwrap().unwrap();
        let mut arg2_1 = args.pop_front().unwrap().unwrap();
        let mut arg3_1 = args.pop_front().unwrap().unwrap();
        // frame 3
        let mut frame = frames.pop().unwrap();
        assert_eq!(Type::CallRequestContinue, *frame.frame_type());
        let mut call_req = CallContinue::decode(frame.payload_mut()).unwrap();
        assert_eq!(Flags::NONE, *call_req.flags());
        let args = call_req.args_mut().args_mut();
        assert_eq!(1, args.len());
        let mut arg3_2 = args.pop_front().unwrap().unwrap();
        // verify args
        let len = arg1_1.len() + arg1_2.len();
        assert_eq!(arg1, arg1_1.chain(arg1_2).copy_to_bytes(len));
        assert_eq!(arg2, arg2_1);
        let len = arg3_1.len() + arg3_2.len();
        assert_eq!(arg3, arg3_1.chain(arg3_2).copy_to_bytes(len));
    }
}
