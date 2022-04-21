#[cfg(test)]
#[macro_use]
extern crate log;
#[macro_use]
extern crate serial_test;

use std::sync::Arc;

use bytes::Bytes;
use test_case::test_case;

use tchannel_rs::handler::{HandlerResult, RequestHandler};
use tchannel_rs::messages::MessageChannel;
use tchannel_rs::messages::RawMessage;
use tchannel_rs::Config;
use tchannel_rs::{SubChannel, TChannel, TResult};

#[test_case("service", "endpoint", "header", "body";    "Basic")]
#[test_case("service", "endpoint", "header", "";        "Empty body")]
#[test_case("service", "endpoint", "", "body";          "Empty header")]
#[test_case("service", "endpoint", "", "";              "Empty header and body")]
#[test_case("service", "a", "b", "c";                   "One byte frame args")]
#[test_case("service", "", "", "";                      "Zero byte frame args")]
#[serial]
#[tokio::test]
async fn single_frame_msg(
    service: &str,
    endpoint: &str,
    header: &str,
    body: &str,
) -> Result<(), anyhow::Error> {
    echo_test(service, endpoint, header, body).await
}

#[test_case("service", "from_v(&['a' as u8; u16::MAX as usize * 10])", "header", "body";        "Long endpoint/arg1")]
#[test_case("service", "endpoint", from_v(&[b'b'; u16::MAX as usize * 10]), "body";             "Long header/arg2")]
#[test_case("service", "endpoint", "header", "from_v(&['c' as u8; u16::MAX as usize * 10])";    "Long body/arg3")]
#[test_case("service",
        "from_v(&['a' as u8; u16::MAX as usize * 10])",
        "from_v(&['b' as u8; u16::MAX as usize * 10])",
        "from_v(&['c' as u8; u16::MAX as usize * 10])";
        "Long all args")]
#[serial]
#[tokio::test]
async fn multi_frame_msg(
    service: &str,
    endpoint: &str,
    header: &str,
    body: &str,
) -> Result<(), anyhow::Error> {
    echo_test(service, endpoint, header, body).await
}

async fn echo_test(
    service: &str,
    endpoint: &str,
    header: &str,
    body: &str,
) -> Result<(), anyhow::Error> {
    // GIVEN
    let _ = env_logger::builder().is_test(true).try_init();
    let server = start_echo_server(service, endpoint)
        .await
        .expect("Failed to start server");
    let req = RawMessage::new(
        endpoint.to_string(),
        header.to_string(),
        Bytes::from(body.to_string()),
    );

    // WHEN
    let res = make_request(service, req.clone())
        .await
        .expect("Failed to make request");
    server.shutdown_server().expect("Failed to shutdown server");

    // THEN
    assert_eq!(
        req.endpoint(),
        res.endpoint(),
        "Endpoint fields should match"
    );
    assert_eq!(req.header(), res.header(), "Header fields should match");
    assert_eq!(req.body(), res.body(), "Body fields should match");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial]
async fn parallel_messages() -> Result<(), anyhow::Error> {
    let service = "service";
    let endpoint = "endpoint";
    let server = start_echo_server(service, endpoint).await?;
    let client = TChannel::new(Config::default())?;
    let subchannel = client.subchannel(&service).await?;

    let small_msgs = (0..50)
        .map(|i| {
            RawMessage::new(
                endpoint.to_string(),
                format!("header-{}", i),
                Bytes::from(format!("body-{}", i)),
            )
        })
        .collect::<Vec<RawMessage>>();
    let large_msgs = (0..5_u8)
        .map(|i| {
            RawMessage::new(
                endpoint.to_string(),
                from_v(&[i as u8; u16::MAX as usize * 20]).to_string(),
                Bytes::from(from_v(&[i + 5_u8; u16::MAX as usize * 10]).to_string()),
            )
        })
        .collect::<Vec<RawMessage>>();

    let (small, large) = tokio::join!(
        tokio::spawn(send_msgs(subchannel.clone(), small_msgs)),
        tokio::spawn(send_msgs(subchannel.clone(), large_msgs))
    );

    assert!(small.is_ok());
    assert!(large.is_ok());
    server.shutdown_server()?;
    Ok(())
}

async fn send_msgs(
    subchannel: Arc<SubChannel>,
    msgs: Vec<RawMessage>,
) -> Result<(), anyhow::Error> {
    for req in msgs {
        debug!("Sending {} bytes.", req.header().len() + req.body().len());
        let res = subchannel.send(req.clone(), &LOCAL_SERVER).await?;
        assert_eq!(req.endpoint(), res.endpoint(), "Endpoints should match");
        assert_eq!(req.header(), res.header(), "Header fields should match");
        assert_eq!(req.body(), res.body(), "Body fields should match");
        debug!("Sent");
    }
    Ok(())
}

async fn start_echo_server<STR: AsRef<str>>(service: STR, endpoint: STR) -> TResult<TChannel> {
    let server = TChannel::new(Config::default())?;
    let subchannel = server.subchannel(&service).await?;
    subchannel.register(&endpoint, EchoHandler {}).await?;
    server.start_server()?;
    Ok(server)
}

async fn make_request<STR: AsRef<str>>(service: STR, req: RawMessage) -> HandlerResult<RawMessage> {
    debug!("Outgoing arg2/header len {}", &req.header().len());
    let client = TChannel::new(Config::default())?;
    let subchannel = client.subchannel(service).await?;
    subchannel.send(req, LOCAL_SERVER).await
}

#[derive(Debug)]
struct EchoHandler {}

impl RequestHandler for EchoHandler {
    type REQ = RawMessage;
    type RES = RawMessage;
    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
        debug!("Incoming arg2/header len {}", request.header().len());
        Ok(request)
    }
}

// Utils

const LOCAL_SERVER: &str = "127.0.0.1:8888";

fn from_v(v: &[u8]) -> &str {
    std::str::from_utf8(v).unwrap()
}
