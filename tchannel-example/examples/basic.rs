use std::collections::HashMap;
use tchannel::channel::*;
use tchannel::messages::raw::*;
use tchannel::messages::*;
use tchannel::Error;
use tchannel::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut tchannel = TChannelBuilder::default().build().unwrap();

    let subchannel_name = "sub_channel";
    let subchannel = tchannel.makeSubchannel(subchannel_name);

    let base = BaseBuilder::default()
        .transportHeaders(HashMap::new())
        .build()
        .unwrap();
    let request = RawRequestBuilder::default().base(base).build().unwrap();

    let response = subchannel.send(&request, "192.168.50.172", 8888).unwrap();

    println!("Respose: TODO");
    Ok(())
}
