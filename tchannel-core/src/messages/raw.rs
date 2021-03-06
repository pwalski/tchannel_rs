use crate::messages::*;
use std::collections::HashMap;

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct RawRequest {
    id: String,
    base: Base,
}

impl Message for RawRequest {}

impl Request for RawRequest {}

pub struct RawResponse {}

impl Message for RawResponse {}

impl Response for RawResponse {}
