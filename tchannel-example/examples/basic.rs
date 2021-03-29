use std::collections::HashMap;
use tchannel::channel::*;
use tchannel::messages::raw::*;
use tchannel::messages::*;
use tchannel::Error;
use tchannel::Result;

use tokio::net::lookup_host;

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
    let addr = lookup_host("192.168.50.172").await.unwrap().next()?;
    let response: RawResponse = subchannel.send(request, addr, 8888).unwrap();

    println!("Respose: {:?}", response);
    Ok(())
}
