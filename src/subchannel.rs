use crate::connection::pool::ConnectionPools;
use crate::connection::{FrameInput, FrameOutput};
use crate::defragmentation::ResponseDefragmenter;
use crate::errors::{CodecError, ConnectionError, HandlerError, TChannelError};
use crate::fragmentation::Fragmenter;
use crate::frames::payloads::ResponseCode;
use crate::frames::TFrameStream;
use crate::handler::{
    MessageArgsHandler, RequestHandler, RequestHandlerAdapter, RequestHandlerAsync,
    RequestHandlerAsyncAdapter,
};
use crate::messages::{Message, MessageArgs, MessageArgsResponse};
use futures::join;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

type HandlerRef = Arc<Mutex<Box<dyn MessageArgsHandler>>>;

#[derive(Debug, new)]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[new(default)]
    handlers: RwLock<HashMap<String, HandlerRef>>,
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
        self.register_handler(endpoint, Arc::new(Mutex::new(Box::new(handler_adapter))))
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
        self.register_handler(endpoint, Arc::new(Mutex::new(Box::new(handler_adapter))))
            .await
    }

    async fn register_handler<S: AsRef<str>>(
        &mut self,
        endpoint: S,
        request_handler: HandlerRef,
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
        let (frames_in, frames_out) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let response = ResponseDefragmenter::new(frames_in)
            .read_response_msg()
            .await;
        frames_out.close().await; //TODO ugly
        response
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameInput, FrameOutput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_frames_io().await?)
    }

    async fn create_frames(&self, request: MessageArgs) -> Result<TFrameStream, TChannelError> {
        Fragmenter::new(self.service_name.clone(), request.arg_scheme, request.args).create_frames()
    }

    pub(crate) async fn handle(&self, request: MessageArgs) -> MessageArgsResponse {
        let endpoint = Self::read_endpoint_name(&request)?;
        let handler_locked = self.get_handler(endpoint).await?;
        let mut handler = handler_locked.lock().await; //TODO do I really want Mutex? maybe handle(&self,..) instead of handle(&mut self,..) ?
        let (_response_code, _message_args) = handler.handle(request).await?;
        todo!()
    }

    async fn get_handler(&self, endpoint: String) -> Result<HandlerRef, TChannelError> {
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
