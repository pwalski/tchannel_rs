use crate::connection::pool::{ConnectionPools, ConnectionPoolsBuilder};
use crate::connection::{ConnectionOptions, FrameInput, FrameOutput};
use crate::defragmentation::Defragmenter;
use crate::errors::{ConnectionError, HandlerError, TChannelError};
use crate::fragmentation::Fragmenter;
use crate::frames::payloads::ResponseCode;
use crate::frames::TFrameStream;
use crate::handler::{
    MessageArgsHandler, RequestHandler, RequestHandlerAdapter, RequestHandlerAsync,
    RequestHandlerAsyncAdapter,
};
use crate::messages::{Message, MessageArgs};
use futures::join;
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

    pub async fn subchannel<STR: AsRef<str>>(
        &mut self,
        service_name: STR,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        if let Some(subchannel) = self.subchannels.read().await.get(service_name.as_ref()) {
            return Ok(subchannel.clone());
        }
        self.make_subchannel(service_name).await
    }

    async fn make_subchannel<STR: AsRef<str>>(
        &self,
        service_name: STR,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        let mut subchannels = self.subchannels.write().await;
        match subchannels.get(service_name.as_ref()) {
            Some(subchannel) => Ok(subchannel.clone()),
            None => {
                debug!("Creating subchannel {}", service_name.as_ref());
                let subchannel = Arc::new(SubChannel::new(
                    String::from(service_name.as_ref()),
                    self.connection_pools.clone(),
                ));
                subchannels.insert(String::from(service_name.as_ref()), subchannel.clone());
                Ok(subchannel)
            }
        }
    }
}

#[derive(Debug, new)]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[new(default)]
    handlers: RwLock<HashMap<String, Box<dyn MessageArgsHandler>>>,
}

impl SubChannel {
    /// Register request handler.
    pub async fn register<S: AsRef<str>, REQ, RES, HANDLER>(
        &mut self,
        endpoint: S,
        request_handler: HANDLER,
    ) -> Result<(), HandlerError>
    where
        REQ: Message + 'static,
        RES: Message + 'static,
        HANDLER: RequestHandler<REQ = REQ, RES = RES> + 'static,
    {
        let handler_adapter = RequestHandlerAdapter::new(request_handler);
        self.register_handler(endpoint, Box::new(handler_adapter))
            .await
    }

    /// Register async request handler.
    pub async fn register_async<S: AsRef<str>, REQ, RES, HANDLER>(
        &mut self,
        endpoint: S,
        request_handler: HANDLER,
    ) -> Result<(), HandlerError>
    where
        REQ: Message + 'static,
        RES: Message + 'static,
        HANDLER: RequestHandlerAsync<REQ = REQ, RES = RES> + 'static,
    {
        let handler_adapter = RequestHandlerAsyncAdapter::new(request_handler);
        self.register_handler(endpoint, Box::new(handler_adapter))
            .await
    }

    async fn register_handler<S: AsRef<str>>(
        &mut self,
        endpoint: S,
        request_handler: Box<dyn MessageArgsHandler>,
    ) -> Result<(), HandlerError> {
        let mut handlers = self.handlers.write().await;
        if handlers.contains_key(endpoint.as_ref()) {
            return Err(HandlerError::RegistrationError(format!(
                "Handler already registered for '{}'",
                endpoint.as_ref()
            )));
        }
        handlers.insert(endpoint.as_ref().to_string(), request_handler);
        Ok(()) //TODO return &mut of nested handler?
    }

    /// Unregister request handler.
    pub async fn unregister<S: AsRef<str>>(&mut self, endpoint: S) -> Result<(), HandlerError> {
        let mut handlers = self.handlers.write().await;
        match handlers.remove(endpoint.as_ref()) {
            Some(_) => Ok(()),
            None => Err(HandlerError::RegistrationError(format!(
                "Handler '{}' is missing.",
                endpoint.as_ref()
            ))),
        }
    }

    pub(super) async fn send<REQ: Message, RES: Message>(
        &self,
        request: REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, RES), crate::errors::TChannelError> {
        let (connection_res, frames_res) =
            join!(self.connect(host), self.create_frames(request.try_into()?));
        let (frames_out, frames_in) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let response = Defragmenter::new(frames_in).create_response().await;
        frames_out.close().await; //TODO ugly
        response
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameOutput, FrameInput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_frame_io().await)
    }

    async fn create_frames(&self, request: MessageArgs) -> Result<TFrameStream, TChannelError> {
        Fragmenter::new(self.service_name.clone(), request.arg_scheme, request.args).create_frames()
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
