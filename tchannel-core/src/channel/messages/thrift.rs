use crate::channel::messages::*;

use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct ThriftRequest {
    base: Base,
}

impl Message for ThriftRequest {}

impl Request for ThriftRequest {}

#[derive(Debug)]
pub struct ThriftResponse {}

impl Message for ThriftResponse {}

impl Response for ThriftResponse {}