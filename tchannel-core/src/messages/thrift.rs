use crate::messages::*;

use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct ThriftRequest<'a> {
    pub value: String,
    pub transportHeaders: HashMap<&'a str, &'a String>,
}

impl Message for ThriftRequest<'_> {

}

impl Request for ThriftRequest<'_> {

}

pub struct ThriftResponse {}

impl Message for ThriftResponse {

}

impl Response for ThriftResponse {

}

