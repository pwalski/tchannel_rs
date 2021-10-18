use log::{error, info};
use std::ops::AddAssign;
use std::time::Duration;
use tchannel_protocol::errors::HandlerError;
use tchannel_protocol::handler::{HandlerResult, RequestHandler};
use tchannel_protocol::messages::raw::RawMessage;
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
    let tchannel = TChannel::new(Config::default())?;
    let subchannel = tchannel.subchannel("server".to_owned()).await?;
    subchannel.register("pong", PongHandler::default()).await?;
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

    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
        info!("Received {:?}", request);
        self.counter.add_assign(1);
        if request.header() != "Marco" {
            return Err(HandlerError::GeneralError("Bad header".into()));
        }
        match self.counter {
            1 => Ok(msg("Pong!".into())),
            2 => Ok(msg("I feel bad ...".into())),
            _ => Err(HandlerError::MessageError(msg("I feel sick ...".into()))),
        }
    }
}

fn msg(msg: String) -> RawMessage {
    RawMessage::new("pong".into(), "Polo".into(), msg.into())
}
