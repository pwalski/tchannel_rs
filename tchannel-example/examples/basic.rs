use log::{debug, error};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use tchannel::channel::connection::ConnectionOptions;
use tchannel::channel::messages::raw::*;
use tchannel::channel::messages::*;
use tchannel::channel::*;
use tchannel::Error;
use tokio::net::lookup_host;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    env_logger::init();
    if let Err(err) = run().await {
        error!("Failure: {:?}", err);
        return Err(err);
    }
    Ok(())
}

async fn run() -> Result<(), Error> {
    let mut tchannel = TChannel::new(ConnectionOptions::default())?;
    let subchannel = tchannel.subchannel(String::from("sub_channel")).await?;
    let requestBase = BaseRequestBuilder::default()
        .id(0) //should not set it here
        .transport_headers(HashMap::new())
        .value(String::from("test"))
        .build()?;
    let request = RawMessageBuilder::default().base(requestBase).build()?;
    let addr = SocketAddr::from_str("192.168.50.172:8888")?;
    debug!("sending");
    match subchannel.send(request, addr).await {
        Ok(response) => debug!("Response: {:?}", response),
        Err(error) => debug!("Fail: {:?}", error),
    }
    Ok(())
}
