use crate::errors::{HandlerError, TChannelError};
use crate::messages::ResponseCode;
use crate::messages::{Message, MessageArgs, MessageArgsResponse};
use futures::{future, Future};
use std::fmt::Debug;
use std::pin::Pin;

/// Trait for handling requests asynchronously.
///
/// Handler can be registered under an `endpoint` name by calling [`SubChannel::register_async`] method.
pub trait RequestHandlerAsync: Debug + Sync + Send {
    type REQ: Message;
    type RES: Message;
    fn handle(
        &mut self,
        request: Self::REQ,
    ) -> Pin<Box<dyn Future<Output = HandlerResult<Self::RES>> + Send + '_>>;
}

/// Trait for handling requests.
///
/// Handler can be registered under an `endpoint` name by calling [`SubChannel::register`] method.
pub trait RequestHandler: Debug + Sync + Send {
    type REQ: Message;
    type RES: Message;
    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES>;
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
            convert(self.0.handle(request).await)
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
        convert(self.0.handle(request))
    }
}

fn convert<MSG: Message>(msg_res: HandlerResult<MSG>) -> MessageArgsResponse {
    match msg_res {
        Ok(message) => Ok((ResponseCode::Ok, message.try_into()?)),
        Err(err) => match err {
            HandlerError::MessageError(message) => Ok((ResponseCode::Ok, message.try_into()?)),
            HandlerError::GeneralError(message) => Err(TChannelError::Error(message)),
            HandlerError::TChannelError(err) => Err(err),
        },
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

pub type HandlerResult<MES> = Result<MES, HandlerError<MES>>;
