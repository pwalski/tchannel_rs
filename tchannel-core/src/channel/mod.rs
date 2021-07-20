pub mod connection;
pub mod frames;
pub mod messages;

use crate::channel::connection::{
    ConnectionOptions, ConnectionPools, ConnectionPoolsBuilder, FrameInput, FrameOutput,
};
use crate::channel::frames::headers::TransportHeader::CallerNameKey;
use crate::channel::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::channel::frames::payloads::ResponseCode;
use crate::channel::frames::payloads::{
    CallArgs, CallContinue, CallFieldsEncoded, CallRequest, CallRequestFields, CallResponse,
    ChecksumType, Codec, Flags, TraceFlags, Tracing, ARG_LEN_LEN,
};
use crate::channel::frames::Type::CallResponseContinue;
use crate::channel::frames::{
    TFrame, TFrameId, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH,
};
use crate::channel::messages::fragmenting::MessageFragmenter;
use crate::channel::messages::{Message, Request, Response};
use crate::handlers::RequestHandler;
use crate::{Error, TChannelError};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::channel::oneshot::Sender;
use futures::join;
use futures::task::Poll;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use futures::{FutureExt, Stream};
use log::{debug, error, info, warn};
use std::collections::{HashMap, VecDeque};
use std::convert::{Infallible, TryFrom, TryInto};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::codec::{Decoder, Encoder, Framed};

pub struct TChannel {
    subchannels: RwLock<HashMap<String, Arc<SubChannel>>>,
    connection_pools: Arc<ConnectionPools>,
}

impl TChannel {
    pub fn new(connection_options: ConnectionOptions) -> Result<Self, TChannelError> {
        let connection_pools = ConnectionPoolsBuilder::default()
            .connection_options(connection_options)
            .build()?;
        Ok(TChannel {
            subchannels: RwLock::new(HashMap::new()),
            connection_pools: Arc::new(connection_pools),
        })
    }

    pub async fn subchannel(
        &mut self,
        service_name: String,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        if let Some(subchannel) = self.subchannels.read().await.get(&service_name) {
            return Ok(subchannel.clone());
        }
        self.make_subchannel(service_name).await
    }

    async fn make_subchannel(
        &self,
        service_name: String,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        let mut subchannels = self.subchannels.write().await;
        match subchannels.get(&service_name) {
            Some(subchannel) => Ok(subchannel.clone()),
            None => {
                debug!("Creating subchannel {}", service_name);
                let subchannel = Arc::new(
                    SubChannelBuilder::default()
                        .service_name(service_name.to_owned())
                        .connection_pools(self.connection_pools.clone())
                        .build()?,
                );
                subchannels.insert(service_name, subchannel.clone());
                Ok(subchannel)
            }
        }
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[builder(setter(skip))]
    handlers: HashMap<String, Box<RequestHandler>>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request, RES: Response>(
        &self,
        request: REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, RES), crate::TChannelError> {
        let (connection_res, frames_res) = join!(
            self.connect(host),
            MessageFragmenter::new(request, self.service_name.clone()).create_frames(),
        );
        let (frames_out, mut frames_in) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let (code, mut args) = read_response(&mut frames_in, RES::arg_scheme()).await?;
        frames_out.close(); //TODO ugly
        debug!("Received args {:?}", args);
        Ok((code, RES::try_from(args)?))
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameOutput, FrameInput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_message_io().await)
    }
}

// TODO separate fragmentation related code

async fn send_frames(frames: TFrameStream, frames_out: &FrameOutput) -> Result<(), TChannelError> {
    debug!("Sending frames");
    frames
        .then(|frame| frames_out.send(frame))
        .inspect_err(|err| error!("Failed to send frame {:?}", err))
        .try_for_each(|res| future::ready(Ok(())))
        .await
}

// defragmentation

async fn read_response(
    frame_input: &mut FrameInput,
    arg_scheme: ArgSchemeValue,
) -> Result<(ResponseCode, Vec<Bytes>), TChannelError> {
    let mut args_defragmenter = ArgsDefragmenter::default();
    let (code, flags) =
        read_response_begin(frame_input, arg_scheme, &mut args_defragmenter).await?;
    if !flags.contains(Flags::MORE_FRAGMENTS_FOLLOW) {
        return Ok((code, args_defragmenter.args()));
    }
    read_response_continue(frame_input, &mut args_defragmenter).await?;
    Ok((code, args_defragmenter.args()))
}

async fn read_response_begin(
    frame_input: &mut FrameInput,
    arg_scheme: ArgSchemeValue,
    args_defragmenter: &mut ArgsDefragmenter,
) -> Result<(ResponseCode, Flags), TChannelError> {
    if let Some(frame_id) = frame_input.recv().await {
        debug!("Received response id: {}", frame_id.id());
        let mut frame = frame_id.frame;
        let response = CallResponse::decode(frame.payload_mut())?;
        verify_headers(response.fields().headers(), arg_scheme)?;
        verify_args(response.args).map(|args| args_defragmenter.add(args))?;
        Ok((response.fields.code, response.flags))
    } else {
        Err(TChannelError::Error("Got no response".to_owned()))
    }
}

async fn read_response_continue(
    frame_input: &mut FrameInput,
    args_defragmenter: &mut ArgsDefragmenter,
) -> Result<(), TChannelError> {
    while let Some(frame_id) = frame_input.recv().await {
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

fn verify_headers(
    headers: &HashMap<String, String>,
    expected_arg_scheme: ArgSchemeValue,
) -> Result<(), TChannelError> {
    if let Some(scheme) = headers.get(TransportHeader::ArgSchemeKey.to_string().as_str()) {
        //TODO ugly
        if !scheme.eq(expected_arg_scheme.to_string().as_str()) {
            return Err(TChannelError::Error(format!(
                "Expected arg scheme '{}' received '{}'",
                expected_arg_scheme.to_string(),
                scheme
            )));
        }
    } else {
        return Err(TChannelError::Error("Missing arg schema arg".to_owned()));
    }
    Ok(())
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
            let mut previous_arg = self.args.pop().ok_or_else(|| BytesMut::new()).unwrap();
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
