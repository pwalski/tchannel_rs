use crate::channel::frames::headers::ArgSchemeValue;
use crate::channel::messages::*;
use bytes::Bytes;

#[derive(Default, Debug)]
pub struct ThriftMessage {}

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

impl Message for ThriftMessage {
    fn arg_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Thrift
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

impl Request for ThriftMessage {}
