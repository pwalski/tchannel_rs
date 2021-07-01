use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::frames::{TFrame, TFrameStream};
use crate::TChannelError;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::net::SocketAddr;
use strum_macros::ToString;

pub mod raw;
pub mod thrift;

pub enum ResponseCode {}

pub trait Message: Debug + Sized + Send {
    fn arg_scheme() -> ArgSchemeValue;
    fn arg1(&self) -> Bytes;
    fn arg2(&self) -> Bytes;
    fn arg3(&self) -> Bytes;
}

pub trait Request: Message {}

pub trait Response: Message + TryFrom<TFrameStream, Error = TChannelError> {}

#[async_trait]
pub trait MessageChannel {
    type REQ: Request;
    type RES: Response + TryFrom<TFrameStream, Error = TChannelError>;

    async fn send(&self, request: Self::REQ, host: SocketAddr) -> Result<Self::RES, TChannelError>;
}
