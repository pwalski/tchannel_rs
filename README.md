[![build status](https://github.com/pwalski/tchannel-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/pwalski/tchannel-rust/actions)

# tchannel_protocol

TChannel is a network multiplexing and framing RPC protocol created by Uber ([protocol specs](https://github.com/uber/tchannel/blob/master/docs/protocol.md)).

#### Disclaimer

The project serves as an excuse to learn Rust therefore the implementation may be suboptimal and features are not tested properly.

### Overview

Features of TChannel protocol implemented so far:

 * [x] A request/response model,
 * [x] Multiplexing multiple requests across the same TCP socket,
 * [x] Out-of-order responses,
 * [ ] Streaming requests and responses,
 * [ ] Checksums of frame args (only None),
 * [ ] Transport of arbitrary payloads:
    * [ ] Thrift
    * [ ] SThrift (streaming Thrift)
    * [ ] JSON
    * [ ] HTTP
    * [x] Raw
 * [ ] Routing mesh
 * [ ] Tracing

Other TODOs:

 * [ ] Proper tests (right now only few happy paths)
 * [ ] Request response TTL
 * [ ] Cancel request
 * [ ] Claim requests

### Examples
```rust
use tchannel_protocol::errors::TChannelError;
use tchannel_protocol::handler::{RequestHandler, Response};
use tchannel_protocol::messages::raw::RawMessage;
use tchannel_protocol::messages::MessageChannel;
use tchannel_protocol::{Config, TChannel};
use tokio::runtime::Runtime;

fn main() -> Result<(), TChannelError> {
    let tserver = Runtime::new().unwrap().block_on(run())?;
    // Shutdown outside of async
    Ok(tserver.shutdown_server())
}

async fn run() -> Result<TChannel, TChannelError> {
    // Server
    let mut tserver = TChannel::new(Config::default())?;
    let subchannel = tserver.subchannel("service".to_string()).await?;
    subchannel.register("endpoint", Handler {}).await?;
    tserver.start_server()?;

    // Client
    let tclient = TChannel::new(Config::default())?;
    let subchannel = tclient.subchannel("service".to_string()).await?;
    let request = RawMessage::new("endpoint".into(), "a".into(), "b".into());
    let response_res = subchannel.send(request, "127.0.0.1:8888").await;

    assert!(response_res.is_ok());
    let response = response_res.unwrap();
    assert_eq!("a", response.header());
    assert_eq!("y".as_bytes(), response.body().as_ref());
    Ok(tserver)
}

#[derive(Debug)]
struct Handler {}
impl RequestHandler for Handler {
    type REQ = RawMessage;
    type RES = RawMessage;
    fn handle(&mut self, request: Self::REQ) -> Response<Self::RES> {
        let req_header = request.header().clone();
        Ok(RawMessage::new("x".into(), req_header, "y".into()))
    }
}
```


## Build

Update of README
```shell
cargo install cargo-readme
cargo readme > README.md
```

## Examples Subproject

Sample server:
```shell
RUST_LOG=DEBUG cargo run --example server
```

Sample `tchannel-java` server:
```shell
mvn -f examples-server package exec:exec -Pserver
```

Sample client:
```shell
RUST_LOG=DEBUG cargo run --example client
```

---

License: MIT OR Apache-2.0
