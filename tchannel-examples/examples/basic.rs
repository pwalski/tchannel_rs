use log::{debug, error};

use std::net::SocketAddr;
use std::str::FromStr;
use tchannel::messages::raw::RawMessage;
use tchannel::messages::MessageChannel;
use tchannel::ConnectionOptions;
use tchannel::TChannel;

type Error = Box<dyn std::error::Error + Send + Sync>;

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
    let subchannel = tchannel.subchannel("server".to_owned()).await?;
    let request = RawMessage::new("pong".into(), "Marco".into(), "Ping!".into());
    let addr = SocketAddr::from_str("192.168.50.172:8888")?;
    debug!("sending");
    match subchannel.send(request, addr).await {
        Ok(response) => debug!("Response: {:?}", response),
        Err(error) => debug!("Fail: {:?}", error),
    }
    Ok(())
}
