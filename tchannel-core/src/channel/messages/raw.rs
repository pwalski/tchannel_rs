use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::frames::{TFrame, TFrameStream};
use crate::channel::messages::*;
use crate::channel::SubChannel;
use crate::Error;
use crate::TChannelError::FrameCodecError;
use bytes::Bytes;
use bytes::BytesMut;
use futures::future::Ready;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;
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

impl TryFrom<Vec<Bytes>> for RawMessage {
    type Error = TChannelError;
    fn try_from(stream: Vec<Bytes>) -> Result<Self, Self::Error> {
        println!("Ending");
        Ok(RawMessage::new(String::new(), String::new(), Bytes::new()))
    }
}

impl TryInto<TFrameStream> for RawMessage {
    type Error = TChannelError;
    fn try_into(self) -> Result<TFrameStream, Self::Error> {
        todo!()
    }
}

#[async_trait]
impl MessageChannel for SubChannel {
    type REQ = RawMessage;
    type RES = RawMessage;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Result<Self::RES, crate::TChannelError> {
        self.send(request, host).await
    }
}
