use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use std::convert::{AsRef, TryFrom};
use std::future::Future;

use crate::frame::TFrame;
use crate::TChannelError;
use std::net::SocketAddr;
use strum_macros::ToString;

pub mod raw;
pub mod serializers;
pub mod thrift;

#[derive(ToString, Debug, PartialEq, Eq, Hash)]
pub enum TransportHeader {
    #[strum(serialize = "as")]
    ARG_SCHEME_KEY,
    #[strum(serialize = "cas")]
    CLAIM_AT_START_KEY,
    #[strum(serialize = "caf")]
    CLAIM_AT_FINISH_KEY,
    #[strum(serialize = "cn")]
    CALLER_NAME_KEY,
    #[strum(serialize = "re")]
    RETRY_FLAGS_KEY,
    #[strum(serialize = "se")]
    SPECULATIVE_EXECUTION_KEY,
    #[strum(serialize = "fd")]
    FAILURE_DOMAIN_KEY,
    #[strum(serialize = "sk")]
    SHARD_KEY_KEY,
}

#[derive(ToString, Debug)]
pub enum ArgSchemeValue {
    #[strum(serialize = "raw")]
    RAW,
    #[strum(serialize = "json")]
    JSON,
    #[strum(serialize = "http")]
    HTTP,
    #[strum(serialize = "thrift")]
    THRIFT,
    #[strum(serialize = "sthrift")]
    STREAMING_THRIFT,
}

#[derive(ToString, Debug)]
pub enum RetryFlagValue {
    #[strum(serialize = "n")]
    NO_RETRY,
    #[strum(serialize = "c")]
    RETRY_ON_CONNECTION_ERROR,
    #[strum(serialize = "t")]
    RETRY_ON_TIMEOUT,
}

pub enum ResponseCode {}

pub trait Message {}

pub trait Request: Debug {}

pub trait Response: Debug {}

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct BaseRequest {
    id: i32,
    value: String,
    transportHeaders: HashMap<TransportHeader, String>,
}

impl BaseRequestBuilder {
    pub fn build(mut self) -> ::std::result::Result<BaseRequest, String> {
        let mut baseRequest = Self::set_default_headers(self.build_internal()?);
        return Ok(baseRequest);
    }

    fn set_default_headers(mut baseRequest: BaseRequest) -> BaseRequest {
        let headers = &mut baseRequest.transportHeaders;
        if !(headers.contains_key(&TransportHeader::RETRY_FLAGS_KEY)) {
            headers.insert(
                TransportHeader::RETRY_FLAGS_KEY,
                RetryFlagValue::RETRY_ON_CONNECTION_ERROR.to_string(),
            );
        }
        baseRequest
    }
}

pub struct BaseResponse<BODY> {
    id: i64,
    body: BODY,
    transportHeaders: HashMap<String, String>,
}

pub trait ResponseBuilder<RES: Response> {
    fn build(&self) -> RES;
}

#[async_trait]
pub trait MessageChannel {
    type REQ: Request;
    type RES: Response + TryFrom<TFrame>;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
        port: u16,
    ) -> Result<Self::RES, TChannelError>;
}
