use std::collections::HashMap;
use std::net::SocketAddr;
use tchannel::channel::messages::raw::*;
use tchannel::channel::messages::*;
use tchannel::channel::*;
use tchannel::Error;
use tokio::net::lookup_host;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    let mut tchannel = TChannelBuilder::default().build()?;
    let subchannel = tchannel.make_subchannel("sub_channel")?;
    let requestBase = BaseRequestBuilder::default()
        .transport_headers(HashMap::new())
        .build()
        .unwrap();
    let request = RawMessageBuilder::default()
        .base(requestBase)
        .build()
        .unwrap();
    let addr = SocketAddr::from(([192, 168, 50, 172], 8888));
    match subchannel.send(request, addr, 8888).await {
        Ok(response) => println!("Response: {:?}", response),
        Err(error) => println!("Fail: {:?}", error),
    }
    Ok(())
}
