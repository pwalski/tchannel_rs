use crate::channel::messages::*;

#[derive(Default, Debug)]
pub struct ThriftMessage {
    base: BaseRequest,
}

impl TryFrom<TFrameStream> for ThriftMessage {
    type Error = TChannelError;
    fn try_from(value: TFrameStream) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryInto<TFrameStream> for ThriftMessage {
    type Error = TChannelError;
    fn try_into(self) -> Result<TFrameStream, Self::Error> {
        todo!()
    }
}

impl Message for ThriftMessage {}

impl Request for ThriftMessage {}
