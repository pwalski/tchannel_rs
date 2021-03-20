use crate::messages::{Request, Response};
use crate::Result;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct RequestHandler
{
}

impl RequestHandler {
    pub fn handle<REQ: Request, RES: Response>(&self, request: Box<REQ>) -> Box<RES> {
        // &self.handler.call(request)
        unimplemented!()
    }
}
