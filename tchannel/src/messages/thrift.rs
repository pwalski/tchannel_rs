use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::messages::{Message, Request};
use bytes::Bytes;
use std::convert::TryFrom;

#[derive(Default, Debug)]
pub struct ThriftMessage {}

impl TryFrom<Vec<Bytes>> for ThriftMessage {
    type Error = CodecError;
    fn try_from(args: Vec<Bytes>) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl Message for ThriftMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Thrift
    }

    fn to_args(self) -> Vec<Bytes> {
        todo!()
    }
}

impl Request for ThriftMessage {}
