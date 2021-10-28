use log::{error, info};
use tchannel_rs::messages::{MessageChannel, RawMessage};
use tchannel_rs::{Config, TChannel};

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
    let tchannel = TChannel::new(Config::default())?;
    let subchannel = tchannel.subchannel("server".to_owned()).await?;
    info!("Sending 3 requests");
    for _ in 0..3 {
        let request = RawMessage::new("pong".into(), "Marco".into(), "Ping!".into());
        match subchannel.send(request, "127.0.0.1:8888").await {
            Ok(response) => info!("Response: {:?}", response),
            Err(error) => info!("Fail: {:?}", error),
        }
    }
    Ok(())
}
