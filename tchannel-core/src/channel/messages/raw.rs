use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::frames::{TFrame, TFrameStream};
use crate::channel::messages::*;
use crate::channel::SubChannel;
use crate::Error;
use crate::TChannelError::FrameCodecError;
use bytes::Bytes;
use bytes::BytesMut;
use futures::Stream;
use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct RawMessage {
    arg1: Bytes,
    arg2: Bytes,
    arg3: Bytes,
}

impl Message for RawMessage {
    fn arg_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Raw
    }

    fn arg1(&self) -> Bytes {
        todo!()
    }

    fn arg2(&self) -> Bytes {
        todo!()
    }

    fn arg3(&self) -> Bytes {
        todo!()
    }
}

impl Request for RawMessage {}

impl Response for RawMessage {}

impl TryFrom<TFrameStream> for RawMessage {
    type Error = TChannelError;
    fn try_from(stream: TFrameStream) -> Result<Self, Self::Error> {
        todo!()
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
