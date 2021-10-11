use crate::channel::TResult;
use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::handler::HandlerResult;
use bytes::Bytes;
use futures::Future;
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::net::ToSocketAddrs;
use std::pin::Pin;

pub mod raw;
#[cfg(feature = "thrift")]
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

    fn send<'a, ADDR: ToSocketAddrs + Send + 'a>(
        &'a self,
        request: Self::REQ,
        host: ADDR,
    ) -> Pin<Box<dyn Future<Output = HandlerResult<Self::RES>> + Send + '_>>;
}

#[derive(Debug, new)]
pub struct MessageArgs {
    pub arg_scheme: ArgSchemeValue,
    pub args: Vec<Bytes>,
}

pub(crate) type MessageArgsResponse = TResult<(ResponseCode, MessageArgs)>;

#[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive)]
pub enum ResponseCode {
    Ok = 0x00,
    Error = 0x01,
}
