use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::SocketAddr;

use async_trait::async_trait;
use bytes::Bytes;

use crate::errors::{CodecError, TChannelError};
use crate::frames::headers::ArgSchemeValue;
use crate::frames::payloads::ResponseCode;

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
