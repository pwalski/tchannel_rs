use std::collections::HashMap;
use tchannel::channel::*;
use tchannel::messages::raw::*;
use tchannel::messages::*;
use tchannel::Error;
use tchannel::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    let service_name = "app";
    let tchannel = TChannelBuilder::new(service_name).build();
    let base = BaseBuilder::default()
        .transportHeaders(HashMap::new())
        .build()
        .unwrap();
    let subchannel_name = "sub_channel";
    let request = RawRequestBuilder::default().base(base).build().unwrap();
    Ok(())
}
