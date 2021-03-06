use std::collections::HashMap;

pub mod raw;
pub mod serializers;
pub mod thrift;

pub trait Message {}

pub trait Request {}

pub trait Response {}

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

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct Base {
    pub value: String,
    pub transportHeaders: HashMap<String, String>,
}
