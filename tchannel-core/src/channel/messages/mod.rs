use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::frames::payloads::ResponseCode;
use crate::error::{CodecError, TChannelError};
use async_trait::async_trait;
use bytes::Bytes;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::SocketAddr;

pub mod defragmenting;
pub mod fragmenting;
pub mod raw;
pub mod thrift;

pub trait Message: Debug + Sized + Send {
    fn args_scheme() -> ArgSchemeValue;
    fn to_args(self) -> Vec<Bytes>;
}

pub trait Request: Message {}

pub trait Response: Message + TryFrom<Vec<Bytes>, Error = CodecError> {}

#[async_trait]
pub trait MessageChannel {
    type REQ: Request;
    type RES: Response;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, Self::RES), TChannelError>;
}
