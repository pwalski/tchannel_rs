use crate::messages::Message;
use async_trait::async_trait;

#[async_trait]
pub trait RequestHandler {
    async fn handle<REQ: Message, RES: Message>(&self, request: REQ) -> RES;
}
