use std::ops::AddAssign;
use std::time::Duration;

use log::{debug, error, info};

use tchannel_protocol::handler::RequestHandler;
use tchannel_protocol::handler::Response;
use tchannel_protocol::messages::raw::RawMessage;
use tchannel_protocol::messages::ResponseCode;
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
    tchannel.start_server()?;
    loop {
        std::thread::sleep(Duration::from_secs(1))
    }
}

#[derive(Debug, Default)]
struct PongHandler {
    counter: u32,
}

impl RequestHandler for PongHandler {
    type REQ = RawMessage;
    type RES = RawMessage;

    fn handle(&mut self, request: Self::REQ) -> Response<Self::RES> {
        info!("Received {:?}", request);
        self.counter.add_assign(1);
        let response = match self.counter {
            1 => RawMessage::new("pong".into(), "Polo".into(), "Pong!".into()),
            2 => RawMessage::new("pong".into(), "Polo".into(), "I feel bad ...".into()),
            _ => RawMessage::new("pong".into(), "Polo".into(), "I feel sick ...".into()),
        };
        Ok((ResponseCode::Ok, response))
    }
}
