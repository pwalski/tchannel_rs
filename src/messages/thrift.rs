use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::messages::Message;
use bytes::Bytes;
use std::convert::TryFrom;

#[derive(Default, Debug)]
pub struct ThriftMessage {}

impl Message for ThriftMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Thrift
    }
}

impl TryFrom<Vec<Bytes>> for ThriftMessage {
    type Error = CodecError;
    fn try_from(_args: Vec<Bytes>) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[allow(clippy::from_over_into)]
impl Into<Vec<Bytes>> for ThriftMessage {
    fn into(self) -> Vec<Bytes> {
        todo!()
    }
}
