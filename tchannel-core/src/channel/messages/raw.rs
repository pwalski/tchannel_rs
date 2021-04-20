use crate::channel::messages::*;
use crate::channel::SubChannel;
use crate::frame::TFrame;

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

impl TryFrom<TFrame> for RawResponse {
    type Error = TChannelError;

    fn try_from(value: TFrame) -> Result<Self, Self::Error> {
        todo!()
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
