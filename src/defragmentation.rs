use std::collections::VecDeque;
use std::str::FromStr;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::connection::FrameInput;
use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::{ArgSchemeValue, TransportHeaderKey};
use crate::frames::payloads::{
    Call, CallArgs, CallContinue, CallFields, CallRequest, CallRequestFields, CallResponse,
    CallResponseFields, ChecksumType, Codec, Flags,
};
use crate::frames::Type;
use crate::messages::{Message, MessageArgs, ResponseCode};

#[derive(Debug)]
pub struct ResponseDefragmenter {
    defragmenter: Defragmenter,
}

impl ResponseDefragmenter {
    pub fn new(frame_input: FrameInput) -> ResponseDefragmenter {
        let defragmenter = Defragmenter::new(frame_input);
        ResponseDefragmenter { defragmenter }
    }

    #[allow(dead_code)]
    pub async fn read_response(self) -> Result<(CallResponseFields, MessageArgs), TChannelError> {
        self.defragmenter
            .read(Type::CallResponse, CallResponse::decode)
            .await
    }

    pub async fn read_response_msg<MSG: Message>(
        self,
    ) -> Result<(ResponseCode, MSG), TChannelError> {
        let (fields, message_args) = self
            .defragmenter
            .read_and_check(Type::CallResponse, CallResponse::decode, |fields| {
                ArgSchemeChecker::new(MSG::args_scheme()).check_args_scheme(fields)
            })
            .await?;
        let msg = MSG::try_from(message_args)?;
        Ok((fields.code, msg))
    }
}

#[derive(Debug)]
pub struct RequestDefragmenter {
    defragmenter: Defragmenter,
}

impl RequestDefragmenter {
    pub fn new(frame_input: FrameInput) -> RequestDefragmenter {
        let defragmenter = Defragmenter::new(frame_input);
        RequestDefragmenter { defragmenter }
    }

    pub async fn read_request(self) -> Result<(CallRequestFields, MessageArgs), TChannelError> {
        self.defragmenter
            .read(Type::CallRequest, CallRequest::decode)
            .await
    }

    #[allow(dead_code)]
    pub async fn read_request_msg<MSG: Message>(self) -> Result<MSG, TChannelError> {
        let _checker = ArgSchemeChecker {
            arg_scheme: MSG::args_scheme(),
        };
        let (_, message_args) = self
            .defragmenter
            .read_and_check(Type::CallRequest, CallRequest::decode, |fields| {
                ArgSchemeChecker::new(MSG::args_scheme()).check_args_scheme(fields)
            })
            .await?;
        Ok(MSG::try_from(message_args)?)
    }
}

#[derive(Debug, new)]
pub struct Defragmenter {
    frame_input: FrameInput,
}

impl Defragmenter {
    async fn read<FIELDS: CallFields, FRAME: Codec + Call<FIELDS>>(
        self,
        frame_type: Type,
        decoder: fn(src: &mut Bytes) -> Result<FRAME, CodecError>,
    ) -> Result<(FIELDS, MessageArgs), TChannelError> {
        self.read_and_check(frame_type, decoder, get_args_scheme)
            .await
    }

    async fn read_and_check<FIELDS: CallFields, FRAME: Codec + Call<FIELDS>>(
        mut self,
        frame_type: Type,
        decode: fn(src: &mut Bytes) -> Result<FRAME, CodecError>,
        check_fields: fn(fields: &FIELDS) -> Result<ArgSchemeValue, CodecError>,
    ) -> Result<(FIELDS, MessageArgs), TChannelError> {
        let mut args_defragmenter = ArgsDefragmenter::default();
        let (fields, flags) = self
            .read_beginning(frame_type, decode, &mut args_defragmenter)
            .await?;
        let args_scheme = check_fields(&fields)?;
        if !flags.contains(Flags::MORE_FRAGMENTS_FOLLOW) {
            let message_args = MessageArgs::new(args_scheme, args_defragmenter.args());
            return Ok((fields, message_args));
        }
        read_continuation(&mut self.frame_input, &mut args_defragmenter).await?;
        let message_args = MessageArgs::new(args_scheme, args_defragmenter.args());
        Ok((fields, message_args))
    }

    async fn read_beginning<FIELDS: CallFields, FRAME: Codec + Call<FIELDS>>(
        &mut self,
        frame_type: Type,
        decoder: fn(src: &mut Bytes) -> Result<FRAME, CodecError>,
        args_defragmenter: &mut ArgsDefragmenter,
    ) -> Result<(FIELDS, Flags), TChannelError> {
        if let Some(frame_id) = self.frame_input.recv().await {
            let mut frame = frame_id.frame;
            if frame_type == frame.frame_type {
                let mut frame = decoder(frame.payload_mut())?; //ugh
                verify_args(frame.args()).map(|args| args_defragmenter.add(args))?;
                let flags = frame.flags();
                Ok((frame.fields(), flags))
            } else {
                Err(TChannelError::Error(format!(
                    "Expected '{:?}' got '{:?}'",
                    frame_type,
                    frame.frame_type()
                )))
            }
        } else {
            Err(TChannelError::Error("Received no response".to_string()))
        }
    }
}

#[derive(Debug, new)]
struct ArgSchemeChecker {
    arg_scheme: ArgSchemeValue,
}

impl ArgSchemeChecker {
    fn check_args_scheme<FIELDS: CallFields>(
        &self,
        fields: &FIELDS,
    ) -> Result<ArgSchemeValue, CodecError> {
        let arg_scheme = get_args_scheme(fields)?;
        if arg_scheme != self.arg_scheme {
            return Err(CodecError::Error(format!(
                "Expected {} got {}",
                self.arg_scheme.to_string(),
                arg_scheme.to_string()
            )));
        }
        Ok(arg_scheme)
    }
}

fn get_args_scheme<FIELDS: CallFields>(fields: &FIELDS) -> Result<ArgSchemeValue, CodecError> {
    let headers = fields.headers();
    if let Some(scheme) = headers.get(TransportHeaderKey::ArgScheme.to_string().as_str()) {
        Ok(ArgSchemeValue::from_str(scheme)?)
    } else {
        Err(CodecError::Error("Missing arg schema arg".to_owned()))
    }
}

async fn read_continuation(
    frame_input: &mut FrameInput,
    args_defragmenter: &mut ArgsDefragmenter,
) -> Result<(), TChannelError> {
    while let Some(frame_id) = frame_input.recv().await {
        let mut frame = frame_id.frame;
        match frame.frame_type() {
            Type::CallResponseContinue | Type::CallRequestContinue => {
                let mut continuation = CallContinue::decode(frame.payload_mut())?;
                let more_follows = continuation.flags().contains(Flags::MORE_FRAGMENTS_FOLLOW); //TODO workaround for borrow checker
                verify_args(continuation.args_mut()).map(|args| args_defragmenter.add(args))?;
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

fn verify_args(call_args: &mut CallArgs) -> Result<&mut VecDeque<Option<Bytes>>, CodecError> {
    match call_args.checksum_type() {
        ChecksumType::None => Ok(call_args.args_mut()),
        _ => todo!(),
    }
}

#[derive(Debug, Default)]
struct ArgsDefragmenter {
    args: Vec<BytesMut>,
}

impl ArgsDefragmenter {
    fn add(&mut self, frame_args: &mut VecDeque<Option<Bytes>>) {
        match frame_args.pop_front() {
            None => return,
            Some(frame_arg) => self.add_first_arg(frame_arg),
        }
        self.add_remaining_args(frame_args);
    }

    fn add_first_arg(&mut self, frame_arg: Option<Bytes>) {
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

    fn add_remaining_args(&mut self, frame_args: &mut VecDeque<Option<Bytes>>) {
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
    use std::collections::HashMap;
    use std::convert::TryInto;

    use tokio_test::*;

    use crate::frames::headers::ArgSchemeValue;
    use crate::frames::headers::TransportHeaderKey::ArgScheme;
    use crate::frames::payloads::{
        CallRequest, CallRequestFields, CallResponseFields, TraceFlags, Tracing,
    };
    use crate::frames::{TFrame, TFrameId};
    use crate::messages::raw::RawMessage;

    use super::*;

    const SERVICE_NAME: &str = "test_service";
    const ARG_SCHEME: ArgSchemeValue = ArgSchemeValue::Raw;

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
        headers.insert(ArgScheme.to_string(), ARG_SCHEME.to_string());
        let call_fields = CallResponseFields::new(ResponseCode::Ok, tracing, headers);
        let request = CallResponse::new(Flags::NONE, call_fields, call_args);
        let frame = TFrame::new(Type::CallResponse, request.encode_bytes().unwrap());
        let frame_id = TFrameId::new(1, frame);
        let (sender, receiver) = tokio::sync::mpsc::channel::<TFrameId>(1);
        let send_result = block_on(sender.send(frame_id));
        assert_ok!(send_result);

        // When
        let defragmenter = ResponseDefragmenter::new(receiver);
        let response_res: Result<(ResponseCode, RawMessage), TChannelError> =
            block_on(defragmenter.read_response_msg());

        // Then
        // assert_ok!(&response_res);
        let (code, response) = response_res.unwrap();
        assert_eq!(ResponseCode::Ok, code);
        assert_eq!("e".to_string(), *response.endpoint());
        assert_eq!("h".to_string(), *response.header());
        assert_eq!(Bytes::from("b"), *response.body());
        let response_args: MessageArgs = response.try_into().unwrap();
        assert_eq!(ARG_SCHEME, response_args.arg_scheme);
        assert_eq!(
            [Bytes::from("e"), Bytes::from("h"), Bytes::from("b")].to_vec(),
            response_args.args
        );
    }

    #[test]
    fn wrong_frame() {
        // Given
        let args = [Bytes::from("a"), Bytes::from("b"), Bytes::from("c")].to_vec();
        let some_args: Vec<Option<Bytes>> = args.into_iter().map(Some).collect();
        let call_args = CallArgs::new(ChecksumType::None, None, VecDeque::from(some_args));
        let tracing = Tracing::new(0, 0, 0, TraceFlags::NONE);
        let mut headers = HashMap::new();
        headers.insert(ArgScheme.to_string(), ARG_SCHEME.to_string());
        let call_fields = CallRequestFields::new(10_000, tracing, SERVICE_NAME.to_owned(), headers);
        let request = CallRequest::new(Flags::NONE, call_fields, call_args);
        let frame = TFrame::new(Type::CallRequest, request.encode_bytes().unwrap());
        let frame_id = TFrameId::new(1, frame);
        let (sender, receiver) = tokio::sync::mpsc::channel::<TFrameId>(1);
        let send_result = block_on(sender.send(frame_id));
        assert_ok!(send_result);

        // When
        let defragmenter = ResponseDefragmenter::new(receiver);
        let response_res: Result<(ResponseCode, RawMessage), TChannelError> =
            block_on(defragmenter.read_response_msg());

        // Then
        assert!(response_res.is_err());
        let res = response_res.err().unwrap();
        assert_eq!(
            TChannelError::Error("Expected 'CallResponse' got 'CallRequest'".to_string()),
            res
        );
    }

    #[test]
    fn multiple_frames() {
        //TODO sigh
    }
}
