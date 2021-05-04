use crate::channel::frames::TFrame;
use crate::channel::messages::*;
use crate::channel::{FrameStream, SubChannel};
use crate::Error;
use crate::TChannelError::FrameCodecError;
use bytes::BytesMut;
use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct RawMessage {
    id: String,
    base: BaseRequest,
}

impl Message for RawMessage {}

impl Request for RawMessage {}

impl Response for RawMessage {}

impl TryFrom<TFrame> for RawMessage {
    type Error = TChannelError;

    fn try_from(value: TFrame) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryInto<TFrame> for RawMessage {
    type Error = TChannelError;

    fn try_into(self) -> Result<TFrame, Self::Error> {
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
        port: u16,
    ) -> Result<Self::RES, crate::TChannelError> {
        self.send(request, host, port).await
    }
}

impl Encoder<RawMessage> for MessageCodec {
    type Error = Error;

    fn encode(&mut self, item: RawMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        todo!()
    }
}

impl Decoder for MessageCodec {
    type Item = RawMessage;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}
