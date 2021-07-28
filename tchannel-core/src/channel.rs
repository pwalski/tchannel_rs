use crate::connection::pool::{ConnectionPools, ConnectionPoolsBuilder};
use crate::connection::{ConnectionOptions, FrameInput, FrameOutput};
use crate::defragmentation::Defragmenter;
use crate::errors::{ConnectionError, TChannelError};
use crate::fragmentation::Fragmenter;
use crate::frames::payloads::ResponseCode;
use crate::frames::TFrameStream;
use crate::handler::RequestHandler;
use crate::messages::{Request, Response};
use futures::future::lazy;
use futures::join;
use futures::FutureExt;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    //TODO handle handlers
    // #[builder(setter(skip))]
    // handlers: HashMap<String, Box<RequestHandler>>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        unimplemented!()
    }

    pub(super) async fn send<REQ: Request, RES: Response>(
        &self,
        request: REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, RES), crate::errors::TChannelError> {
        let (connection_res, frames_res) = join!(self.connect(host), self.create_frames(request));
        let (frames_out, frames_in) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let response = Defragmenter::new(frames_in).read_response().await;
        frames_out.close().await; //TODO ugly
        response
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameOutput, FrameInput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_frame_io().await)
    }

    async fn create_frames<REQ: Request>(
        &self,
        request: REQ,
    ) -> Result<TFrameStream, TChannelError> {
        Fragmenter::new(
            self.service_name.clone(),
            REQ::args_scheme(),
            request.to_args(),
        )
        .create_frames()
    }
}

async fn send_frames(
    frames: TFrameStream,
    frames_out: &FrameOutput,
) -> Result<(), ConnectionError> {
    debug!("Sending frames");
    frames
        .then(|frame| frames_out.send(frame))
        .inspect_err(|err| error!("Failed to send frame {:?}", err))
        .try_for_each(|_res| future::ready(Ok(())))
        .await
}
