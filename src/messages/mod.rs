use crate::channel::TResult;
use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::handler::HandlerResult;
use crate::messages::args::MessageWithArgs;
use bytes::Bytes;
use futures::Future;
use std::fmt::Debug;
use std::net::ToSocketAddrs;
use std::pin::Pin;

#[cfg(feature = "json")]
mod json;
mod raw;
#[cfg(feature = "thrift")]
mod thrift;

#[cfg(feature = "json")]
pub use json::{JsonMessage, JsonMessageBuilder};
pub use raw::{RawMessage, RawMessageBuilder};
#[cfg(feature = "thrift")]
pub use thrift::ThriftMessage;

pub trait Message: MessageWithArgs + Debug + Send {}

pub trait MessageChannel<REQ: Message, RES: Message> {
    /// Sends `message` to `host` address.
    ///
    /// Error message response arrives as [`super::errors::HandlerError::MessageError`].
    /// # Arguments
    /// * `request` - Implementation of `Message` trait.
    /// * `host` - Address used to connect to host or find previously pooled connection.
    fn send<'a, ADDR: ToSocketAddrs + Send + 'a>(
        &'a self,
        request: REQ,
        host: ADDR,
    ) -> Pin<Box<dyn Future<Output = HandlerResult<RES>> + Send + '_>>;
}

pub(crate) mod args {
    use super::*;

    #[doc(hidden)]
    pub trait MessageWithArgs:
        TryFrom<MessageArgs, Error = CodecError> + TryInto<MessageArgs, Error = CodecError>
    {
        fn args_scheme() -> ArgSchemeValue;
    }

    #[doc(hidden)]
    #[derive(Debug, new)]
    pub struct MessageArgs {
        pub arg_scheme: ArgSchemeValue,
        pub args: Vec<Bytes>,
    }

    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive)]
    pub enum ResponseCode {
        Ok = 0x00,
        Error = 0x01,
    }

    pub type MessageArgsResponse = TResult<(ResponseCode, MessageArgs)>;
}
