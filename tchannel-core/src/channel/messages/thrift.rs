use crate::channel::messages::*;

#[derive(Default, Debug)]
pub struct ThriftRequest {
    base: BaseRequest,
}

impl Message for ThriftRequest {}

impl Request for ThriftRequest {}

#[derive(Debug)]
pub struct ThriftResponse {}

impl Message for ThriftResponse {}

impl Response for ThriftResponse {}
