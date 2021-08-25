use crate::errors::TChannelError;
use crate::frames::payloads::ResponseCode;
use crate::messages::{Message, MessageArgs, MessageArgsResponse};
use futures::{future, Future};
use std::fmt::Debug;
use std::pin::Pin;

pub type Response<RES> = Result<(ResponseCode, RES), TChannelError>;

/// Trait for handling requests asynchronously.
///
/// Handler can be registered under an `endpoint` name by calling [`SubChannel::register_async`] method.
pub trait RequestHandlerAsync: Debug + Sync + Send {
    type REQ: Message;
    type RES: Message;
    fn handle(
        &mut self,
        request: Self::REQ,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>>;
}

/// Trait for handling requests.
///
/// Handler can be registered under an `endpoint` name by calling [`SubChannel::register`] method.
pub trait RequestHandler: Debug + Sync + Send {
    type REQ: Message;
    type RES: Message;
    fn handle(&self, request: Self::REQ) -> Response<Self::RES>;
}

pub(crate) trait MessageArgsHandler: Debug + Send + Sync {
    fn handle(
        &mut self,
        request: MessageArgs,
    ) -> Pin<Box<dyn Future<Output = MessageArgsResponse> + Send + '_>>;
}

#[derive(Debug, new)]
pub(crate) struct RequestHandlerAsyncAdapter<
    REQ: Message,
    RES: Message,
    HANDLER: RequestHandlerAsync<REQ = REQ, RES = RES>,
>(HANDLER);

impl<REQ: Message, RES: Message, HANDLER: RequestHandlerAsync<REQ = REQ, RES = RES>>
    MessageArgsHandler for RequestHandlerAsyncAdapter<REQ, RES, HANDLER>
{
    fn handle(
        &mut self,
        request_args: MessageArgs,
    ) -> Pin<Box<dyn Future<Output = MessageArgsResponse> + Send + '_>> {
        Box::pin(async move {
            let request = REQ::try_from(request_args)?;
            let (code, message) = self.0.handle(request).await?;
            let message_args: MessageArgs = message.try_into()?;
            Ok((code, message_args))
        })
    }
}

#[derive(Debug, new)]
pub(crate) struct RequestHandlerAdapter<
    REQ: Message,
    RES: Message,
    HANDLER: RequestHandler<REQ = REQ, RES = RES>,
>(HANDLER);

impl<REQ: Message, RES: Message, HANDLER: RequestHandler<REQ = REQ, RES = RES>>
    RequestHandlerAdapter<REQ, RES, HANDLER>
{
    pub fn handle(&mut self, request_args: MessageArgs) -> MessageArgsResponse {
        let request = REQ::try_from(request_args)?;
        let (code, message) = self.0.handle(request)?;
        let message_args: MessageArgs = message.try_into()?;
        Ok((code, message_args))
    }
}

impl<REQ: Message, RES: Message, HANDLER: RequestHandler<REQ = REQ, RES = RES>> MessageArgsHandler
    for RequestHandlerAdapter<REQ, RES, HANDLER>
{
    fn handle(
        &mut self,
        request_args: MessageArgs,
    ) -> Pin<Box<dyn Future<Output = MessageArgsResponse> + Send + '_>> {
        Box::pin(future::ready(self.handle(request_args)))
    }
}
