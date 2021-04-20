use crate::frame::TFrame;
use crate::TChannelError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::SocketAddr;
use strum_macros::ToString;

pub mod raw;
pub mod thrift;

#[derive(ToString, Debug, PartialEq, Eq, Hash)]
pub enum TransportHeader {
    #[strum(serialize = "as")]
    ArgSchemeKey,
    #[strum(serialize = "cas")]
    ClaimAtStartKey,
    #[strum(serialize = "caf")]
    ClaimAtFinishKey,
    #[strum(serialize = "cn")]
    CallerNameKey,
    #[strum(serialize = "re")]
    RetryFlagsKey,
    #[strum(serialize = "se")]
    SpeculativeExecutionKey,
    #[strum(serialize = "fd")]
    FailureDomainKey,
    #[strum(serialize = "sk")]
    ShardKeyKey,
}

#[derive(ToString, Debug)]
pub enum ArgSchemeValue {
    #[strum(serialize = "raw")]
    Raw,
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "http")]
    Http,
    #[strum(serialize = "thrift")]
    Thrift,
    #[strum(serialize = "sthrift")]
    StreamingThrift,
}

#[derive(ToString, Debug)]
pub enum RetryFlagValue {
    #[strum(serialize = "n")]
    NoRetry,
    #[strum(serialize = "c")]
    RetryOnConnectionErrot,
    #[strum(serialize = "t")]
    RetryOnTimeout,
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
    transport_headers: HashMap<TransportHeader, String>,
}

impl BaseRequestBuilder {
    pub fn build(mut self) -> ::std::result::Result<BaseRequest, String> {
        let base_request = Self::set_default_headers(self.build_internal()?);
        return Ok(base_request);
    }

    fn set_default_headers(mut base_request: BaseRequest) -> BaseRequest {
        let headers = &mut base_request.transport_headers;
        if !(headers.contains_key(&TransportHeader::RetryFlagsKey)) {
            headers.insert(
                TransportHeader::RetryFlagsKey,
                RetryFlagValue::RetryOnConnectionErrot.to_string(),
            );
        }
        base_request
    }
}

pub struct BaseResponse<BODY> {
    id: i64,
    body: BODY,
    transport_headers: HashMap<String, String>,
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
