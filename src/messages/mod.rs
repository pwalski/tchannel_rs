use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::ArgSchemeValue;
use crate::handler::Response;
use bytes::Bytes;
use futures::Future;
use num_traits::FromPrimitive;
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

#[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive)]
pub enum ResponseCode {
    Ok = 0x00,
    Error = 0x01,
}
