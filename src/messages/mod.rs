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

mod raw;
#[cfg(feature = "thrift")]
mod thrift;

pub use raw::RawMessage;
#[cfg(feature = "thrift")]
pub use thrift::ThriftMessage;

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

    /// Sends `message` to `host` address.
    ///
    /// Error message response arrives as [`super::errors::HandlerError::MessageError`].
    /// # Arguments
    /// * `request` - Implementation of `Message` trait.
    /// * `host` - Address used to connect to host or find previously pooled connection.
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
pub(crate) enum ResponseCode {
    Ok = 0x00,
    Error = 0x01,
}
