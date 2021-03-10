use crate::messages::{Request, Response};
use crate::Result;
use std::fmt::Debug;

pub trait RequestHandler: Debug {
    fn handle(&self, request: &dyn Request) -> Result<Box<dyn Response>>;
}
