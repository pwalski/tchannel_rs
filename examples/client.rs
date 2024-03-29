/**
# Besides of `tchannel_rs` the example requires following dependencies:
tokio =  { version = "^1", features = ["macros"] }
log = "^0"
env_logger = "^0" # to print logs
 */
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
    let subchannel = tchannel.subchannel("server").await?;
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
