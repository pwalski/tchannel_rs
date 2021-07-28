use crate::connection::FrameInput;
use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::TransportHeader;
use crate::frames::payloads::{
    CallArgs, CallContinue, CallResponse, ChecksumType, Codec, Flags, ResponseCode,
};
use crate::frames::Type;
use crate::messages::{Message, Response};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;

#[derive(Debug, new)]
pub struct Defragmenter<RES: Response> {
    frame_input: FrameInput,
    #[new(default)]
    resource_type: PhantomData<RES>,
}

impl<RES: Response> Defragmenter<RES> {
    pub async fn read_response(mut self) -> Result<(ResponseCode, RES), TChannelError> {
        let mut args_defragmenter = ArgsDefragmenter::default();
        let (code, flags) = self.read_response_begin(&mut args_defragmenter).await?;
        if !flags.contains(Flags::MORE_FRAGMENTS_FOLLOW) {
            return Ok((code, RES::try_from(args_defragmenter.args())?));
        }
        self.read_response_continue(&mut args_defragmenter).await?;
        let response = RES::try_from(args_defragmenter.args())?;
        Ok((code, response))
    }

    async fn read_response_begin(
        &mut self,
        args_defragmenter: &mut ArgsDefragmenter,
    ) -> Result<(ResponseCode, Flags), TChannelError> {
        if let Some(frame_id) = self.frame_input.recv().await {
            debug!("Received response id: {}", frame_id.id());
            let mut frame = frame_id.frame;
            if *frame.frame_type() != Type::CallResponse {
                Err(CodecError::Error(format!(
                    "Expected '{:?}' got '{:?}'",
                    Type::CallResponse,
                    frame.frame_type()
                )))?
            }
            let response = CallResponse::decode(frame.payload_mut())?;
            self.verify_headers(response.fields().headers())?;
            verify_args(response.args).map(|args| args_defragmenter.add(args))?;
            Ok((response.fields.code, response.flags))
        } else {
            Err(TChannelError::Error("Got no response".to_owned()))
        }
    }

    async fn read_response_continue(
        &mut self,
        args_defragmenter: &mut ArgsDefragmenter,
    ) -> Result<(), TChannelError> {
        while let Some(frame_id) = self.frame_input.recv().await {
            let mut frame = frame_id.frame;
            match frame.frame_type() {
                Type::CallResponseContinue => {
                    let continuation = CallContinue::decode(frame.payload_mut())?;
                    let more_follows = continuation.flags().contains(Flags::MORE_FRAGMENTS_FOLLOW); //TODO workaround for borrow checker
                    verify_args(continuation.args).map(|args| args_defragmenter.add(args))?;
                    if more_follows {
                        break;
                    }
                }
                Type::Error => {
                    debug!("Transport error: {:?}", frame);
                    return Err(TChannelError::Error(format!(
                        "Transport error: {:?}",
                        frame
                    )));
                }
                frame_type => {
                    debug!("Unexpected frame: {:?}", frame);
                    return Err(TChannelError::Error(format!(
                        "Unexpected frame: {:?}",
                        frame_type
                    )));
                }
            };
        }
        Ok(())
    }

    fn verify_headers(&self, headers: &HashMap<String, String>) -> Result<(), TChannelError> {
        if let Some(scheme) = headers.get(TransportHeader::ArgSchemeKey.to_string().as_str()) {
            //TODO ugly
            if !scheme.eq(RES::args_scheme().to_string().as_str()) {
                return Err(TChannelError::Error(format!(
                    "Expected arg scheme '{}' received '{}'",
                    RES::args_scheme().to_string(),
                    scheme
                )));
            }
        } else {
            return Err(TChannelError::Error("Missing arg schema arg".to_owned()));
        }
        Ok(())
    }
}

fn verify_args(call_args: CallArgs) -> Result<VecDeque<Option<Bytes>>, TChannelError> {
    match call_args.checksum_type() {
        ChecksumType::None => Ok(call_args.args),
        _ => todo!(),
    }
}

#[derive(Debug, Default)]
struct ArgsDefragmenter {
    args: Vec<BytesMut>,
}

impl ArgsDefragmenter {
    fn add(&mut self, mut frame_args: VecDeque<Option<Bytes>>) {
        match frame_args.pop_front() {
            None => return,
            Some(frame_arg) => self.add_first_arg(frame_arg),
        }
        self.add_remaining_args(frame_args);
    }

    fn add_first_arg(&mut self, mut frame_arg: Option<Bytes>) {
        match frame_arg {
            Some(frame_arg_bytes) => match self.args.pop() {
                Some(mut previous_incomplete_arg) => {
                    previous_incomplete_arg.put(frame_arg_bytes);
                    self.args.push(previous_incomplete_arg);
                }
                None => {
                    let first_arg = BytesMut::from(frame_arg_bytes.chunk());
                    self.args.push(first_arg);
                }
            },
            // next arg (according to protocol specs an empty arg at the beginning means the previous arg ended at the end of previous frame)
            None => self.args.push(BytesMut::new()),
        }
    }

    fn add_remaining_args(&mut self, mut frame_args: VecDeque<Option<Bytes>>) {
        for frame_arg in frame_args {
            match frame_arg {
                None => self.args.push(BytesMut::new()), // empty arg
                Some(fragment) => {
                    self.args.push(BytesMut::from(fragment.chunk()));
                }
            }
        }
    }

    fn args(&mut self) -> Vec<Bytes> {
        self.args
            .iter_mut()
            .map(|arg_mut| Bytes::from(arg_mut.split_to(arg_mut.len())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::headers::ArgSchemeValue;
    use crate::frames::headers::TransportHeader::ArgSchemeKey;
    use crate::frames::payloads::{
        CallRequest, CallRequestFields, CallResponseFields, TraceFlags, Tracing,
    };
    use crate::frames::{TFrame, TFrameId};
    use crate::messages::raw::RawMessage;
    use crate::messages::Message;
    use tokio::sync::mpsc::Receiver;
    use tokio_test::*;

    const service_name: &str = "test_service";
    const arg_scheme: ArgSchemeValue = ArgSchemeValue::Raw;

    #[test]
    fn single_frame() {
        // Given
        let args: Vec<Option<Bytes>> = ["e", "h", "b"]
            .to_vec()
            .into_iter()
            .map(|s| Some(Bytes::from(s)))
            .collect();
        let call_args = CallArgs::new(ChecksumType::None, None, VecDeque::from(args));
        let tracing = Tracing::new(0, 0, 0, TraceFlags::NONE);
        let mut headers = HashMap::new();
        headers.insert(ArgSchemeKey.to_string(), arg_scheme.to_string());
        let call_fields = CallResponseFields::new(ResponseCode::Ok, tracing, headers);
        let request = CallResponse::new(Flags::NONE, call_fields, call_args);
        let frame = TFrame::new(Type::CallResponse, request.encode_bytes().unwrap());
        let frame_id = TFrameId::new(1, frame);
        let (sender, receiver) = tokio::sync::mpsc::channel::<TFrameId>(1);
        block_on(sender.send(frame_id));

        // When
        let defragmenter = Defragmenter::new(receiver);
        let response_res: Result<(ResponseCode, RawMessage), TChannelError> =
            block_on(defragmenter.read_response());

        // Then
        // assert_ok!(&response_res);
        let (code, response) = response_res.unwrap();
        assert_eq!(ResponseCode::Ok, code);
        assert_eq!("e".to_string(), *response.endpoint());
        assert_eq!("h".to_string(), *response.header());
        assert_eq!(Bytes::from("b"), *response.body());
        assert_eq!(
            [Bytes::from("e"), Bytes::from("h"), Bytes::from("b")].to_vec(),
            response.to_args()
        );
    }

    #[test]
    fn wrong_frame() {
        // Given
        let args = [Bytes::from("a"), Bytes::from("b"), Bytes::from("c")].to_vec();
        let some_args: Vec<Option<Bytes>> = args.clone().into_iter().map(|b| Some(b)).collect();
        let call_args = CallArgs::new(ChecksumType::None, None, VecDeque::from(some_args));
        let tracing = Tracing::new(0, 0, 0, TraceFlags::NONE);
        let mut headers = HashMap::new();
        headers.insert(ArgSchemeKey.to_string(), arg_scheme.to_string());
        let call_fields = CallRequestFields::new(10_000, tracing, service_name.to_owned(), headers);
        let request = CallRequest::new(Flags::NONE, call_fields, call_args);
        let frame = TFrame::new(Type::CallRequest, request.encode_bytes().unwrap());
        let frame_id = TFrameId::new(1, frame);
        let (sender, receiver) = tokio::sync::mpsc::channel::<TFrameId>(1);
        block_on(sender.send(frame_id));

        // When
        let defragmenter = Defragmenter::new(receiver);
        let response_res: Result<(ResponseCode, RawMessage), TChannelError> =
            block_on(defragmenter.read_response());

        // Then
        assert!(response_res.is_err());
        let res = response_res.err().unwrap();
        assert_eq!(
            TChannelError::CodecError(CodecError::Error(format!(
                "Expected 'CallResponse' got 'CallRequest'"
            ))),
            res
        );
    }

    #[test]
    fn multiple_frames() {
        //TODO sigh
    }
}
