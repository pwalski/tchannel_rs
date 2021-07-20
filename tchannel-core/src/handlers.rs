use crate::channel::messages::{Request, Response};
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct RequestHandler {}

impl RequestHandler {
    pub fn handle<REQ: Request, RES: Response>(&self, request: Box<REQ>) -> Box<RES> {
        unimplemented!()
    }
}
