use crate::channel::messages::*;
use std::collections::HashMap;
use crate::channel::SubChannel;

#[derive(Default, Debug, Builder, Getters)]
#[builder(pattern = "owned")]
pub struct RawRequest {
    id: String,
    base: Base,
}

impl Message for RawRequest {}

impl Request for RawRequest {}

#[derive(Debug)]
pub struct RawResponse {}

impl Message for RawResponse {}

impl Response for RawResponse {}

#[async_trait]
impl MessageChannel<RawRequest, RawResponse> for SubChannel {
    async fn send(&self, request: RawRequest, host: SocketAddr, port: u16) -> Result<RawResponse, Error> {
        let peer = self.peers_pool.get_or_add(host);
        // let peer = self.peers_pool.get_or_add(host);
        // match self.peers_pool.try_borrow_mut() {
        //     Ok(pool) => pool.print_shit()
        // }
        println!("done");
        unimplemented!()
    }
}
