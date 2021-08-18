use crate::channel::SubChannel;
use crate::errors::{CodecError};
use crate::frames::headers::ArgSchemeValue;
use crate::messages::{Message, MessageChannel, Response};
use bytes::{Buf, Bytes};
use futures::Future;
use std::collections::VecDeque;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::pin::Pin;
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
}

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

#[allow(clippy::from_over_into)]
impl Into<Vec<Bytes>> for RawMessage {
    fn into(self) -> Vec<Bytes> {
        Vec::from([self.endpoint.into(), self.header.into(), self.body])
    }
}

fn bytes_to_string(arg: Option<Bytes>) -> Result<String, FromUtf8Error> {
    arg.map_or_else(
        || Ok(String::new()),
        |b| String::from_utf8(Vec::from(b.chunk())),
    )
}

impl MessageChannel for SubChannel {
    type REQ = RawMessage;
    type RES = RawMessage;

    fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>> {
        Box::pin(self.send(request, host))
    }
}
