use log::{error, info};
use std::net::SocketAddr;
use std::str::FromStr;
use tchannel_protocol::messages::raw::RawMessage;
use tchannel_protocol::messages::MessageChannel;
use tchannel_protocol::Config;
use tchannel_protocol::TChannel;

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
    let mut tchannel = TChannel::new(Config::default())?;
    let subchannel = tchannel.subchannel("server".to_owned()).await?;
    let request = RawMessage::new("pong".into(), "Marco".into(), "Ping!".into());
    // let addr = SocketAddr::from_str("192.168.50.172:8888")?;
    let addr = SocketAddr::from_str("127.0.0.1:8888")?;
    info!("sending");
    match subchannel.send(request, addr).await {
        Ok(response) => info!("Response: {:?}", response),
        Err(error) => info!("Fail: {:?}", error),
    }
    Ok(())
}