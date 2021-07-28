use crate::messages::{Request, Response};
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait RequestHandler {
    async fn handle<REQ: Request, RES: Response>(&self, request: REQ) -> RES;
}
