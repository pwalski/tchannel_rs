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
use crate::channel::messages::defragmenting::Defragmenter;
use crate::channel::messages::fragmenting::Fragmenter;
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
            Fragmenter::new(request, self.service_name.clone()).create_frames(),
        );
        let (frames_out, mut frames_in) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let response = Defragmenter::new(frames_in).read_response().await;
        frames_out.close().await; //TODO ugly
        return response;
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameOutput, FrameInput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_message_io().await)
    }
}

async fn send_frames(frames: TFrameStream, frames_out: &FrameOutput) -> Result<(), TChannelError> {
    debug!("Sending frames");
    frames
        .then(|frame| frames_out.send(frame))
        .inspect_err(|err| error!("Failed to send frame {:?}", err))
        .try_for_each(|res| future::ready(Ok(())))
        .await
}
