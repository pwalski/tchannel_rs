use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::ArgSchemeValue;
use crate::frames::payloads::ResponseCode;
use bytes::Bytes;
use futures::Future;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::pin::Pin;

pub mod raw;
pub mod thrift;

pub trait Message:
    Debug + Sized + Send + TryFrom<Vec<Bytes>, Error = CodecError> + Into<Vec<Bytes>>
{
    fn args_scheme() -> ArgSchemeValue;
}

type Response<RES> = Result<(ResponseCode, RES), TChannelError>;

pub trait MessageChannel {
    type REQ: Message;
    type RES: Message;

    fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>>;
}
