use log::{error};


use std::time::Duration;


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
    let _subchannel = tchannel.subchannel("server".to_owned()).await?;
    tchannel.start_server()?;
    loop {
        std::thread::sleep(Duration::from_secs(1))
    }
}
