use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::frames::{TFrame, TFrameStream};
use crate::channel::messages::*;
use crate::channel::SubChannel;
use crate::Error;
use crate::TChannelError::FrameCodecError;
use bytes::BytesMut;
use bytes::{Buf, Bytes};
use futures::future::Ready;
use futures::{Stream, StreamExt};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::future::Future;
use std::string::FromUtf8Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default, Debug, Builder, Getters, new)]
#[builder(pattern = "owned")]
pub struct RawMessage {
    //arg1
    endpoint: String,
    //arg2
    header: String,
    //arg3
    body: Bytes,
}

impl RawMessage {}

impl Message for RawMessage {
    fn arg_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Raw
    }

    fn args(self) -> Vec<Bytes> {
        Vec::from([self.endpoint.into(), self.header.into(), self.body])
    }
}

impl Request for RawMessage {}

impl Response for RawMessage {}

//TODO use it or drop it
impl TryFrom<Vec<Bytes>> for RawMessage {
    type Error = TChannelError;
    fn try_from(mut args: Vec<Bytes>) -> Result<Self, Self::Error> {
        let mut deq_args = VecDeque::from(args);
        Ok(RawMessage::new(
            bytes_to_string(deq_args.pop_front())?,
            bytes_to_string(deq_args.pop_front())?,
            deq_args.pop_front().unwrap_or_else(|| Bytes::new()),
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
    ) -> Result<(ResponseCode, Self::RES), crate::TChannelError> {
        self.send(request, host).await
    }
}
