use std::fmt::Debug;
use strum_macros::EnumString;
use strum_macros::ToString;

#[derive(ToString, Debug, PartialEq, Eq, Hash)]
pub enum TransportHeaderKey {
    #[strum(serialize = "as")]
    ArgScheme,
    #[allow(dead_code)]
    #[strum(serialize = "cas")]
    ClaimAtStart,
    #[allow(dead_code)]
    #[strum(serialize = "caf")]
    ClaimAtFinish,
    #[allow(dead_code)]
    #[strum(serialize = "cn")]
    CallerName,
    #[allow(dead_code)]
    #[strum(serialize = "re")]
    RetryFlags,
    #[allow(dead_code)]
    #[strum(serialize = "se")]
    SpeculativeExecution,
    #[allow(dead_code)]
    #[strum(serialize = "fd")]
    FailureDomain,
    #[allow(dead_code)]
    #[strum(serialize = "sk")]
    ShardKey,
}

#[derive(ToString, Debug, EnumString, PartialEq, Eq)]
pub enum ArgSchemeValue {
    #[strum(serialize = "raw")]
    Raw,
    #[allow(dead_code)]
    #[strum(serialize = "json")]
    Json,
    #[allow(dead_code)]
    #[strum(serialize = "http")]
    Http,
    #[allow(dead_code)]
    #[strum(serialize = "thrift")]
    Thrift,
    #[allow(dead_code)]
    #[strum(serialize = "sthrift")]
    StreamingThrift,
    //TODO how to handle it?
    #[allow(dead_code)]
    #[strum(disabled)]
    Custom(String),
}

#[derive(ToString, Debug, PartialEq, Eq)]
pub enum RetryFlagValue {
    #[allow(dead_code)]
    #[strum(serialize = "n")]
    NoRetry,
    #[allow(dead_code)]
    #[strum(serialize = "c")]
    RetryOnConnectionError,
    #[allow(dead_code)]
    #[strum(serialize = "t")]
    RetryOnTimeout,
}

#[derive(ToString, Debug, EnumString, PartialEq, Eq)]
pub enum InitHeaderKey {
    #[allow(dead_code)]
    #[strum(serialize = "host_port")]
    HostPort,
    #[allow(dead_code)]
    #[strum(serialize = "process_name")]
    ProcessName,
    #[allow(dead_code)]
    #[strum(serialize = "tchannel_language")]
    TChannelLanguage,
    #[allow(dead_code)]
    #[strum(serialize = "tchannel_language_version")]
    TChannelLanguageVersion,
    #[allow(dead_code)]
    #[strum(serialize = "tchannel_version")]
    TChannelVersion,
}
