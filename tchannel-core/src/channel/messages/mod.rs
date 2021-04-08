use crate::Error;
use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use std::net::SocketAddr;

pub mod raw;
pub mod serializers;
pub mod thrift;

pub mod headers {
    pub static ARG_SCHEME_KEY: &str = "as";
    pub static CLAIM_AT_START_KEY: &str = "cas";
    pub static CLAIM_AT_FINISH_KEY: &str = "caf";
    pub static CALLER_NAME_KEY: &str = "cn";
    pub static RETRY_FLAGS_KEY: &str = "re";
    pub static SPECULATIVE_EXECUTION_KEY: &str = "se";
    pub static FAILURE_DOMAIN_KEY: &str = "fd";
    pub static SHARD_KEY_KEY: &str = "sk";
}

pub enum ResponseCode {}

pub trait Message {}

pub trait Request: Debug {}

pub trait Response: Debug {}

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct BaseRequest {
    value: String,
    transportHeaders: HashMap<String, String>,
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
