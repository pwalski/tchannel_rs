use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::messages::{Message, MessageArgs};
use std::convert::{TryFrom, TryInto};

#[derive(Default, Debug)]
pub struct ThriftMessage {}

impl Message for ThriftMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Thrift
    }
}

impl TryFrom<MessageArgs> for ThriftMessage {
    type Error = CodecError;
    fn try_from(_args: MessageArgs) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[allow(clippy::from_over_into)]
impl TryInto<MessageArgs> for ThriftMessage {
    type Error = CodecError;
    fn try_into(self) -> Result<MessageArgs, Self::Error> {
        todo!()
    }
}
