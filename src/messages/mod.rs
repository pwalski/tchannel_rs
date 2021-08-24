use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::ArgSchemeValue;
use crate::frames::payloads::ResponseCode;
use crate::handler::Response;
use bytes::Bytes;
use futures::Future;
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::net::SocketAddr;
use std::pin::Pin;

pub mod raw;
pub mod thrift;

pub trait Message:
    Debug
    + Sized
    + Send
    + TryFrom<MessageArgs, Error = CodecError>
    + TryInto<MessageArgs, Error = CodecError>
{
    fn args_scheme() -> ArgSchemeValue;
}

pub trait MessageChannel {
    type REQ: Message;
    type RES: Message;

    fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>>;
}

#[derive(Debug, new)]
pub struct MessageArgs {
    pub arg_scheme: ArgSchemeValue,
    pub args: Vec<Bytes>,
}

pub(crate) type MessageArgsResponse = Result<(ResponseCode, MessageArgs), TChannelError>;
