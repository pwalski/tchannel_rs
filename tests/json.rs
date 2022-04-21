#[cfg(test)]
use tchannel_rs::handler::{HandlerResult, RequestHandler};
#[cfg(feature = "json")]
use tchannel_rs::messages::JsonMessage;
use tchannel_rs::messages::MessageChannel;
use tchannel_rs::Config;
use tchannel_rs::{TChannel, TResult};

#[cfg(feature = "json")]
#[tokio::test]
async fn test() {
    // GIVEN
    let _ = env_logger::builder().is_test(true).try_init();
    let service = "service".to_string();
    let method_name = "test_method".to_string();
    let server = start_echo_server(service, method_name.clone())
        .await
        .expect("Failed to start server");
    let service = "service";
    let headers = r#"
        {
            "header_1": "foo",
            "header_2": 2
        }"#;
    let headers = serde_json::from_str(headers).expect("Incorrect JSON");
    let body = r#"
        {
            "field_1": "foo",
            "field_2": 2,
            "field_3" : {
                "field_31": 1,
                "field_32": "two"
            }
        }"#;
    let body = serde_json::from_str(body).expect("Incorrect JSON");
    let req = JsonMessage::new(method_name, headers, body);

    // WHEN
    let res = make_request(service, req.clone())
        .await
        .expect("Request failed");
    server.shutdown_server().expect("Failed to shutdown server");

    // THEN
    assert_eq!(
        req.method_name(),
        res.method_name(),
        "Method_name fields should match"
    );
    assert_eq!(req.headers(), res.headers(), "Headers fields should match");
    assert_eq!(req.body(), res.body(), "Body fields should match");
}

#[cfg(feature = "json")]
async fn start_echo_server<STR: AsRef<str>>(service: STR, method_name: STR) -> TResult<TChannel> {
    let server = TChannel::new(Config::default())?;
    let subchannel = server.subchannel(&service).await?;
    subchannel.register(&method_name, EchoHandler {}).await?;
    server.start_server()?;
    Ok(server)
}

#[cfg(feature = "json")]
async fn make_request<STR: AsRef<str>>(
    service: STR,
    req: JsonMessage,
) -> HandlerResult<JsonMessage> {
    let client = TChannel::new(Config::default())?;
    let subchannel = client.subchannel(service).await?;
    subchannel.send(req, LOCAL_SERVER).await
}

#[derive(Debug)]
struct EchoHandler {}

#[cfg(feature = "json")]
impl RequestHandler for EchoHandler {
    type REQ = JsonMessage;
    type RES = JsonMessage;
    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
        Ok(request)
    }
}

// Utils

const LOCAL_SERVER: &str = "127.0.0.1:8888";
