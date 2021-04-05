use std::collections::HashMap;
use tchannel::channel::*;
use tchannel::channel::messages::raw::*;
use tchannel::channel::messages::*;
use tchannel::Error;
use tchannel::Result;

use tokio::net::lookup_host;
use std::net::{SocketAddr};

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut tchannel = TChannelBuilder::default().build().unwrap();

    let subchannel_name = "sub_channel";
    let subchannel = tchannel.make_subchannel(subchannel_name);

    let base = BaseBuilder::default()
        .transportHeaders(HashMap::new())
        .build()
        .unwrap();
    let request = RawRequestBuilder::default().base(base).build().unwrap();
    let addr = SocketAddr::from(([192,168,50,172], 8888));
    match subchannel.send(request, addr, 8888).await {
        Ok(response) => println!("Respose: {:?}", response),
        Err(error) => println!("Fail: {:?}", error)
    }
    Ok(())
}
