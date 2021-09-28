use crate::channel::TResult;
use crate::connection::pool::ConnectionPools;
use crate::connection::{ConnectionResult, FrameInput, FrameOutput};
use crate::defragmentation::ResponseDefragmenter;
use crate::errors::{CodecError, ConnectionError, HandlerError, TChannelError};
use crate::fragmentation::RequestFragmenter;
use crate::frames::TFrameStream;
use crate::handler::{
    HandlerResult, MessageArgsHandler, RequestHandler, RequestHandlerAdapter, RequestHandlerAsync,
    RequestHandlerAsyncAdapter,
};
use crate::messages::ResponseCode;
use crate::messages::{Message, MessageArgs, MessageArgsResponse};
use futures::join;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

type HandlerRef = Arc<Mutex<Box<dyn MessageArgsHandler>>>;

/// TChannel protocol subchannel.
#[derive(Debug, new)]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[new(default)]
    handlers: RwLock<HashMap<String, HandlerRef>>,
}

impl SubChannel {
    /// Sends `message` to `host` address.
    ///
    /// # Arguments
    /// * `request` - Implementation of `Message` trait
    /// * `host` - Address used to connect to host or find previously pooled connection.
    pub(super) async fn send<REQ: Message, RES: Message, ADDR: ToSocketAddrs>(
        &self,
        request: REQ,
        host: ADDR,
    ) -> HandlerResult<RES> {
        match self.send_internal(request, host).await {
            Ok((code, response)) => match code {
                ResponseCode::Ok => Ok(response),
                ResponseCode::Error => Err(HandlerError::MessageError(response)),
            },
            Err(err) => Err(HandlerError::TChannelError(err)),
        }
    }

    pub(super) async fn send_internal<REQ: Message, RES: Message, ADDR: ToSocketAddrs>(
        &self,
        request: REQ,
        host: ADDR,
    ) -> TResult<(ResponseCode, RES)> {
        let host = first_addr(host)?;
        let (connection_res, frames_res) =
            join!(self.connect(host), self.create_frames(request.try_into()?));
        let (frames_in, frames_out) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let response = ResponseDefragmenter::new(frames_in)
            .read_response_msg()
            .await;
        frames_out.close().await; //TODO ugly
        response
    }

    /// Registers request handler.
    pub async fn register<STR: AsRef<str>, REQ, RES, HANDLER>(
        &self,
        endpoint: STR,
        request_handler: HANDLER,
    ) -> TResult<()>
    where
        REQ: Message + 'static,
        RES: Message + 'static,
        HANDLER: RequestHandler<REQ = REQ, RES = RES> + 'static,
    {
        let handler_adapter = RequestHandlerAdapter::new(request_handler);
        self.register_handler(endpoint, Arc::new(Mutex::new(Box::new(handler_adapter))))
            .await
    }

    /// Registers async request handler.
    pub async fn register_async<STR: AsRef<str>, REQ, RES, HANDLER>(
        &self,
        endpoint: STR,
        request_handler: HANDLER,
    ) -> TResult<()>
    where
        REQ: Message + 'static,
        RES: Message + 'static,
        HANDLER: RequestHandlerAsync<REQ = REQ, RES = RES> + 'static,
    {
        let handler_adapter = RequestHandlerAsyncAdapter::new(request_handler);
        self.register_handler(endpoint, Arc::new(Mutex::new(Box::new(handler_adapter))))
            .await
    }

    /// Unregister request handler.
    pub async fn unregister<S: AsRef<str>>(&mut self, endpoint: S) -> TResult<()> {
        let mut handlers = self.handlers.write().await;
        match handlers.remove(endpoint.as_ref()) {
            Some(_) => Ok(()),
            None => Err(TChannelError::Error(format!(
                "Handler '{}' is missing.",
                endpoint.as_ref()
            ))),
        }
    }

    async fn register_handler<S: AsRef<str>>(
        &self,
        endpoint: S,
        request_handler: HandlerRef,
    ) -> TResult<()> {
        let mut handlers = self.handlers.write().await;
        if handlers.contains_key(endpoint.as_ref()) {
            return Err(TChannelError::Error(format!(
                "Handler already registered for '{}'",
                endpoint.as_ref()
            )));
        }
        handlers.insert(endpoint.as_ref().to_string(), request_handler);
        Ok(()) //TODO return &mut of nested handler?
    }

    async fn connect(&self, host: SocketAddr) -> TResult<(FrameInput, FrameOutput)> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_frames_io().await?)
    }

    async fn create_frames(&self, request_args: MessageArgs) -> TResult<TFrameStream> {
        RequestFragmenter::new(self.service_name.clone(), request_args).create_frames()
    }

    pub(crate) async fn handle(&self, request: MessageArgs) -> MessageArgsResponse {
        let endpoint = Self::read_endpoint_name(&request)?;
        let handler_locked = self.get_handler(endpoint).await?;
        let mut handler = handler_locked.lock().await; //TODO do I really want Mutex? maybe handle(&self,..) instead of handle(&mut self,..) ?
        handler.handle(request).await
    }

    async fn get_handler(&self, endpoint: String) -> TResult<HandlerRef> {
        let handlers = self.handlers.read().await;
        match handlers.get(&endpoint) {
            Some(handler) => Ok(handler.clone()),
            None => Err(TChannelError::Error(format!(
                "No handler with name '{}'.",
                endpoint
            ))),
        }
    }

    fn read_endpoint_name(request: &MessageArgs) -> Result<String, CodecError> {
        match request.args.get(0) {
            Some(arg) => Ok(String::from_utf8(arg.to_vec())?),
            None => Err(CodecError::Error("Missing arg1/endpoint name".to_string())),
        }
    }
}

fn first_addr<ADDR: ToSocketAddrs>(addr: ADDR) -> ConnectionResult<SocketAddr> {
    let mut addrs = addr.to_socket_addrs()?;
    if let Some(addr) = addrs.next() {
        return Ok(addr);
    }
    Err(ConnectionError::Error(
        "Unable to get host addr".to_string(),
    ))
}

async fn send_frames(frames: TFrameStream, frames_out: &FrameOutput) -> ConnectionResult<()> {
    debug!("Sending frames");
    frames
        .then(|frame| frames_out.send(frame))
        .inspect_err(|err| error!("Failed to send frame {:?}", err))
        .try_for_each(|_res| future::ready(Ok(())))
        .await
}
