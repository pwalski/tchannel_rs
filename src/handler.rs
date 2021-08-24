use crate::errors::TChannelError;
use crate::frames::payloads::ResponseCode;
use crate::messages::{Message, MessageArgs, MessageArgsResponse};
use async_trait::async_trait;
use futures::Future;
use std::fmt::Debug;
use std::pin::Pin;

pub type Response<RES> = Result<(ResponseCode, RES), TChannelError>;

#[async_trait]
pub trait RequestHandler: Debug + Sync + Send {
    type REQ: Message;
    type RES: Message;
    fn handle(
        &self,
        request: Self::REQ,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>>;
}

#[derive(Debug, new)]
pub(crate) struct RequestHandlerAdapter<
    REQ: Message,
    RES: Message,
    HANDLER: RequestHandler<REQ = REQ, RES = RES>,
> {
    request_handler: HANDLER,
}

pub(crate) trait MessageArgsHandler: Debug + Send + Sync {
    fn handle(
        &self,
        request: MessageArgs,
    ) -> Pin<Box<dyn Future<Output = MessageArgsResponse> + Send + '_>>;
}

impl<REQ: Message, RES: Message, HANDLER: RequestHandler<REQ = REQ, RES = RES>> MessageArgsHandler
    for RequestHandlerAdapter<REQ, RES, HANDLER>
{
    fn handle(
        &self,
        request_args: MessageArgs,
    ) -> Pin<Box<dyn Future<Output = MessageArgsResponse> + Send + '_>> {
        Box::pin(async move {
            let request = REQ::try_from(request_args)?;
            let (code, message) = self.request_handler.handle(request).await?;
            let message_args: MessageArgs = message.try_into()?;
            Ok((code, message_args))
        })
    }
}
