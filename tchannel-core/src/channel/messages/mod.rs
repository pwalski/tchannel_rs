use crate::Error;
use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use std::convert::AsRef;
use std::future::Future;

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
pub enum ArgScheme {
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

pub enum ResponseCode {}

pub trait Message {}

pub trait Request: Debug {}

pub trait Response: Debug {}

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct BaseRequest {
    value: String,
    transportHeaders: HashMap<TransportHeader, String>,
}

impl BaseRequestBuilder {
    // pub fn argScheme()

    pub fn build(mut self) -> ::std::result::Result<BaseRequest, String> {
        self.build_internal()
    }
}

impl BaseRequest {
    pub fn set_arg_scheme(&mut self, argScheme: ArgScheme) {
        self.transportHeaders
            .insert(TransportHeader::ARG_SCHEME_KEY, argScheme.to_string());
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
    type RES: Response;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
        port: u16,
    ) -> Result<Self::RES, Error>;
}
