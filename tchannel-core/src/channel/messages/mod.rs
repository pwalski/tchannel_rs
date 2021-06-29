use crate::channel::frames::{TFrame, TFrameStream};
use crate::TChannelError;
use async_trait::async_trait;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
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

pub trait Message:
    TryInto<TFrameStream, Error = TChannelError>
    + TryFrom<TFrameStream, Error = TChannelError>
    + Debug
    + Sized
{
}

pub trait Request: Message {}

pub trait Response: Message {}

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct BaseRequest {
    id: i32,
    value: String,
    #[builder(default = "HashMap::new()")]
    transport_headers: HashMap<TransportHeader, String>,
}

impl BaseRequestBuilder {
    pub fn build(mut self) -> ::std::result::Result<BaseRequest, String> {
        let base_request = self.build_internal()?;
        Ok(Self::set_default_headers(base_request))
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
    type REQ: Request + TryInto<TFrameStream, Error = TChannelError>;
    type RES: Response + TryFrom<TFrameStream, Error = TChannelError>;

    async fn send(&self, request: Self::REQ, host: SocketAddr) -> Result<Self::RES, TChannelError>;
}

pub struct MessageCodec {}

enum ArgEncodingStatus {
    //Completely stored in frame
    Complete,
    //Completely stored in frame with maxed frame capacity
    CompleteAtTheEnd,
    //Partially stored in frame
    Incomplete,
}

fn encode_arg(src: &mut Bytes, dst: &mut BytesMut) -> Result<ArgEncodingStatus, TChannelError> {
    let src_remaining = src.remaining();
    let dst_remaining = dst.capacity() - dst.len();
    if src_remaining == 0 {
        Ok(ArgEncodingStatus::Complete)
    } else if let 0..=2 = dst_remaining {
        Ok(ArgEncodingStatus::Incomplete)
    } else if src_remaining + 2 > dst_remaining {
        let fragment = src.split_to(dst_remaining - 2);
        dst.put_u16(fragment.len() as u16);
        dst.put_slice(fragment.as_ref());
        Ok(ArgEncodingStatus::Incomplete)
    } else {
        dst.put_u16(src.len() as u16);
        dst.put_slice(src.as_ref());
        if (src_remaining + 2 == dst_remaining) {
            Ok(ArgEncodingStatus::CompleteAtTheEnd)
        } else {
            Ok(ArgEncodingStatus::Complete)
        }
    }
}
