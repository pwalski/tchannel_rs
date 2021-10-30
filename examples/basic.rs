/**
# Besides of `tchannel_rs` the example requires following dependencies:
tokio =  { version = "^1", features = ["macros"] }
env_logger = "^0" # to print logs
*/
use tchannel_rs::handler::{HandlerResult, RequestHandler};
use tchannel_rs::messages::{MessageChannel, RawMessage};
use tchannel_rs::{Config, TChannel, TResult};

#[tokio::main]
async fn main() -> TResult<()> {
    // To see TChannel logs
    env_logger::init();
    // Server
    let tserver = TChannel::new(Config::default())?;
    let subchannel = tserver.subchannel("service".to_string()).await?;
    subchannel.register("endpoint", Handler {}).await?;
    tserver.start_server()?;

    // Client
    let tclient = TChannel::new(Config::default())?;
    let subchannel = tclient.subchannel("service".to_string()).await?;
    let request = RawMessage::new("endpoint".into(), "a".into(), "b".into());
    let response_res = subchannel.send(request, "127.0.0.1:8888").await;

    // Server shutdown
    tserver.shutdown_server()?;

    assert!(response_res.is_ok());
    let response = response_res.unwrap();
    assert_eq!("a", response.header());
    assert_eq!("y".as_bytes(), response.body().as_ref());
    Ok(())
}

#[derive(Debug)]
struct Handler {}
impl RequestHandler for Handler {
    type REQ = RawMessage;
    type RES = RawMessage;
    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
        let req_header = request.header().clone();
        Ok(RawMessage::new("x".into(), req_header, "y".into()))
    }
}
