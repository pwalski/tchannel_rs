pub mod connection;
pub mod frames;
pub mod messages;

use crate::channel::connection::{
    ConnectionOptions, ConnectionPools, ConnectionPoolsBuilder, FrameOutput,
};
use crate::channel::frames::headers::TransportHeader::CallerNameKey;
use crate::channel::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::channel::frames::payloads::{ChecksumType, Codec, TraceFlags, Tracing};
use crate::channel::frames::{TFrame, TFrameStream, Type};
use crate::channel::messages::{Message, Request, Response};
use crate::handlers::RequestHandler;
use crate::{Error, TChannelError};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::channel::oneshot::Sender;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use futures::{FutureExt, Stream};
use log::{debug, error, info, warn};
use std::collections::HashMap;
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
                        .service_name(service_name.to_string())
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
        let frame_stream = self.create_frames(request);
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

    fn create_frames<REQ: Request>(&self, request: REQ) -> TFrameStream {
        let headers = self.create_headers(REQ::arg_scheme());
        let tracing = self.create_tracing();
        // need headers length before fragmentation - calculate it or serialize it before serializing tframe
        todo!()
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

enum ArgEncodingStatus {
    //Completely stored in frame
    Complete,
    //Completely stored in frame with maxed frame capacity
    CompleteAtTheEnd,
    //Partially stored in frame
    Incomplete,
}

fn encode_arg(src: &mut Bytes, dst: &mut BytesMut) -> Result<ArgEncodingStatus, TChannelError> {
    let src_remaining = src.remaining();
    let dst_remaining = dst.capacity() - dst.len();
    if src_remaining == 0 {
        Ok(ArgEncodingStatus::Complete)
    } else if let 0..=2 = dst_remaining {
        Ok(ArgEncodingStatus::Incomplete)
    } else if src_remaining + 2 > dst_remaining {
        let fragment = src.split_to(dst_remaining - 2);
        dst.put_u16(fragment.len() as u16);
        dst.put_slice(fragment.as_ref());
        Ok(ArgEncodingStatus::Incomplete)
    } else {
        dst.put_u16(src.len() as u16);
        dst.put_slice(src.as_ref());
        if (src_remaining + 2 == dst_remaining) {
            Ok(ArgEncodingStatus::CompleteAtTheEnd)
        } else {
            Ok(ArgEncodingStatus::Complete)
        }
    }
}

fn calculate_checksum(
    arg1: Option<Bytes>,
    arg2: Option<Bytes>,
    arg3: Option<Bytes>,
    csum_type: ChecksumType,
) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
}
