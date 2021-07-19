use std::collections::HashMap;
use std::fmt::Debug;
use strum_macros::EnumString;
use strum_macros::ToString;

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

#[derive(ToString, Debug, EnumString)]
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
