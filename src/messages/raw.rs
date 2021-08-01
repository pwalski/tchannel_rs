use crate::channel::SubChannel;
use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::frames::payloads::ResponseCode;
use crate::messages::{Message, MessageChannel, Request, Response};
use async_trait::async_trait;
use bytes::{Buf, Bytes};
use std::collections::VecDeque;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::string::FromUtf8Error;

#[derive(Default, Debug, Builder, Getters, MutGetters, new)]
#[builder(pattern = "owned")]
pub struct RawMessage {
    //arg1
    #[get = "pub"]
    endpoint: String,
    //arg2
    #[get = "pub"]
    header: String,
    //arg3
    #[get = "pub"]
    #[get_mut = "pub"]
    body: Bytes,
}

impl RawMessage {}

impl Message for RawMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Raw
    }

    fn to_args(self) -> Vec<Bytes> {
        Vec::from([self.endpoint.into(), self.header.into(), self.body])
    }
}

impl Request for RawMessage {}

impl Response for RawMessage {}

//TODO use it or drop it
impl TryFrom<Vec<Bytes>> for RawMessage {
    type Error = CodecError;
    fn try_from(args: Vec<Bytes>) -> Result<Self, Self::Error> {
        let mut deq_args = VecDeque::from(args);
        Ok(RawMessage::new(
            bytes_to_string(deq_args.pop_front())?,
            bytes_to_string(deq_args.pop_front())?,
            deq_args.pop_front().unwrap_or_else(Bytes::new),
        ))
    }
}

fn bytes_to_string(arg: Option<Bytes>) -> Result<String, FromUtf8Error> {
    arg.map_or_else(
        || Ok(String::new()),
        |b| String::from_utf8(Vec::from(b.chunk())),
    )
}

#[async_trait]
impl MessageChannel for SubChannel {
    type REQ = RawMessage;
    type RES = RawMessage;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, Self::RES), crate::errors::TChannelError> {
        self.send(request, host).await
    }
}
