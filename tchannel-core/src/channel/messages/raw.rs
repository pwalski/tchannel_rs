use crate::channel::messages::*;
use crate::channel::SubChannel;
use crate::frame::TFrame;
use std::collections::HashMap;
use std::future::Future;

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct RawRequest {
    id: String,
    base: BaseRequest,
}

impl Message for RawRequest {}

impl Request for RawRequest {}

#[derive(Debug)]
pub struct RawResponse {}

impl Message for RawResponse {}

impl Response for RawResponse {}

impl From<TFrame> for RawResponse {
    fn from(frame: TFrame) -> Self {
        // todo!()
        Self {}
    }
}

#[async_trait]
impl MessageChannel for SubChannel {
    type REQ = RawRequest;
    type RES = RawResponse;

    async fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
        port: u16,
    ) -> Result<Self::RES, crate::TChannelError> {
        self.send(request, host, port).await
    }
}
