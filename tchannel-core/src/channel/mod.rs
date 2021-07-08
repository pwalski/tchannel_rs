pub mod connection;
pub mod frames;
pub mod messages;

use crate::channel::connection::{
    ConnectionOptions, ConnectionPools, ConnectionPoolsBuilder, FrameOutput,
};
use crate::channel::frames::headers::TransportHeader::CallerNameKey;
use crate::channel::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::channel::frames::payloads::{
    CallArgs, CallContinue, CallFieldsEncoded, CallRequest, CallRequestFields, ChecksumType, Codec,
    Flags, TraceFlags, Tracing,
};
use crate::channel::frames::{TFrame, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH};
use crate::channel::messages::{Message, Request, Response};
use crate::channel::FragmentationStatus::{CompleteAtTheEnd, Incomplete};
use crate::handlers::RequestHandler;
use crate::{Error, TChannelError};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::channel::oneshot::Sender;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use futures::{FutureExt, Stream};
use log::{debug, error, info, warn};
use std::collections::{HashMap, VecDeque};
use std::convert::{TryFrom, TryInto};
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
    ) -> Result<RES, crate::TChannelError> {
        let frame_stream = self.create_frames(request)?;
        //TODO link/join futures
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        let (frame_output, frame_input) = connection.send_many().await;
        Self::send_frames(frame_stream, frame_output); // handle failure?
        let str = frame_input.map(|frame_id| frame_id.frame);
        RES::try_from(Box::pin(str))
    }

    async fn send_frames(frame_stream: TFrameStream, frame_output: FrameOutput) -> JoinHandle<()> {
        tokio::spawn(async move {
            frame_stream
                .then(|frame| frame_output.send(frame))
                .inspect_err(|err| warn!("Failed to send frame. Error: {}", err))
                .take_while(|res| future::ready(res.is_ok()));
        })
    }

    fn create_frames<REQ: Request>(&self, request: REQ) -> Result<TFrameStream, TChannelError> {
        let mut args = Vec::from([request.arg3(), request.arg2(), request.arg1()]);

        let mut request_fields = self.create_request_fields_bytes(REQ::arg_scheme())?;
        let payload_limit = Self::calculate_payload_limit(request_fields.len());
        let frame_args = Self::create_frame_args(&mut args, payload_limit);
        let flag = Self::get_frame_flag(&args);

        let mut call_frames = Vec::new();
        let call_request = CallFieldsEncoded::new(flag, request_fields, frame_args);
        call_frames.push(TFrame::new(Type::CallRequest, call_request.encode_bytes()?));

        while !args.is_empty() {
            let payload_limit = Self::calculate_payload_limit(0);
            let frame_args = Self::create_frame_args(&mut args, payload_limit);
            let flag = Self::get_frame_flag(&args);
            let call_continue = CallContinue::new(flag, frame_args);
            call_frames.push(TFrame::new(
                Type::CallRequestContinue,
                call_continue.encode_bytes()?,
            ))
        }

        Ok(Box::pin(futures::stream::iter(call_frames)))
    }

    fn get_frame_flag(remaining_args: &Vec<Bytes>) -> Flags {
        if remaining_args.is_empty() {
            Flags::NONE
        } else {
            Flags::MORE_FRAGMENTS_FOLLOW
        }
    }

    fn create_frame_args(args: &mut Vec<Bytes>, payload_limit: usize) -> CallArgs {
        let frame_args = Self::create_frame_args_vec(args, payload_limit);
        let checksum_type = ChecksumType::None;
        let checksum = calculate_checksum(&frame_args, checksum_type);
        CallArgs::new(checksum_type, checksum, frame_args)
    }

    fn create_frame_args_vec(args: &mut Vec<Bytes>, payload_limit: usize) -> Vec<Option<Bytes>> {
        let mut frame_args = Vec::with_capacity(3);
        let mut remaining_limit = payload_limit;
        while let Some(mut arg) = args.pop() {
            let (status, arg_bytes) = fragment_arg(&mut arg, remaining_limit);
            arg_bytes.map(|arg| match arg.len() {
                0 => frame_args.push(None),
                len => {
                    remaining_limit = remaining_limit - len;
                    frame_args.push(Some(arg));
                }
            });
            if status == Incomplete {
                args.push(arg);
                break;
            } else if status == CompleteAtTheEnd {
                args.push(Bytes::new());
                break;
            }
        }
        frame_args
    }

    fn create_request_fields_bytes(
        &self,
        arg_scheme: ArgSchemeValue,
    ) -> Result<Bytes, TChannelError> {
        let mut bytes = BytesMut::new();
        self.create_request_fields(arg_scheme).encode(&mut bytes)?;
        return Ok(Bytes::from(bytes));
    }

    fn create_request_fields(&self, arg_scheme: ArgSchemeValue) -> CallRequestFields {
        let tracing = self.create_tracing();
        let headers = self.create_headers(arg_scheme);
        //TODO configurable TTL
        CallRequestFields::new(60_000, tracing, self.service_name.clone(), headers)
    }

    fn calculate_payload_limit(fields_len: usize) -> usize {
        //64KiB max frame size - header size -1 (flag) - serialized fields size
        FRAME_MAX_LENGTH as usize - FRAME_HEADER_LENGTH as usize - 1 - fields_len
    }

    fn create_headers(&self, arg_scheme: ArgSchemeValue) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            TransportHeader::ArgSchemeKey.to_string(),
            arg_scheme.to_string(),
        );
        headers.insert(CallerNameKey.to_string(), self.service_name.clone()); //TODO RC? ref+lifetime?
        return headers;
    }

    fn create_tracing(&self) -> Tracing {
        Tracing::new(0, 0, 0, TraceFlags::NONE)
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
    } else if let 0..=2 = payload_limit {
        (FragmentationStatus::Incomplete, None)
    } else if src_remaining + 2 > payload_limit {
        let fragment = arg.split_to(payload_limit - 2);
        let fragment_len = fragment.len() as u16;
        let payload = Bytes::from([&fragment_len.to_be_bytes(), fragment.chunk()].concat());
        (FragmentationStatus::Incomplete, Some(payload))
    } else {
        let arg_len = arg.len() as u16;
        let payload = Bytes::from([&arg_len.to_be_bytes(), arg.chunk()].concat());
        if (src_remaining + 2 == payload_limit) {
            (FragmentationStatus::CompleteAtTheEnd, Some(payload))
        } else {
            (FragmentationStatus::Complete, Some(payload))
        }
    }
}

fn calculate_checksum(args: &Vec<Option<Bytes>>, csum_type: ChecksumType) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
}
