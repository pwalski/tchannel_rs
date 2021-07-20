use crate::channel::connection::FrameInput;
use crate::channel::frames::headers::{TransportHeader};
use crate::channel::frames::payloads::{
    CallArgs, CallContinue, CallResponse, ChecksumType, Codec, Flags, ResponseCode,
};
use crate::channel::frames::Type;
use crate::channel::messages::Response;
use crate::TChannelError;
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
            if !scheme.eq(RES::arg_scheme().to_string().as_str()) {
                return Err(TChannelError::Error(format!(
                    "Expected arg scheme '{}' received '{}'",
                    RES::arg_scheme().to_string(),
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
        if frame_args.is_empty() {
            return;
        } else if let Some(arg) = frame_args.pop_front().unwrap() {
            // First frame arg might be continuation of arg from previous frame
            let mut previous_arg = self.args.pop().ok_or_else(BytesMut::new).unwrap();
            previous_arg.put(arg);
            self.args.push(previous_arg);
        } else {
            self.args.push(BytesMut::new());
        }
        for frame_arg in frame_args {
            match frame_arg {
                None => self.args.push(BytesMut::new()), // begin new arg
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
