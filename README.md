[![build status](https://github.com/pwalski/tchannel_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/pwalski/tchannel_rs/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](./LICENSE.md)
[![crate](https://img.shields.io/crates/v/tchannel_rs.svg)](https://crates.io/crates/tchannel_rs)
[![documentation](https://docs.rs/tchannel_rs/badge.svg)](https://docs.rs/tchannel_rs)

# tchannel_rs

TChannel is a network multiplexing and framing RPC protocol created by Uber ([protocol specs](https://github.com/uber/tchannel/blob/master/docs/protocol.md)).

### Overview

Features of TChannel protocol implemented so far:

 * [x] A request/response model,
 * [x] Multiplexing multiple requests across the same TCP socket,
 * [x] Out-of-order responses,
 * [ ] Streaming requests and responses,
 * [ ] Checksums of frame args (only _None_),
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
 * [ ] Use Tower?
 * [ ] Implement Serde Serialize/Deserialize for Message types

The goal of the project is to provide a similar API to Java TChannel implementation
which is why both connection pools and server task handler are hidden from user.

**Disclaimer**

> It is an unofficial implementation of TChannel protocol.
> The project was used to learn Rust and it still has some missing features,
> so it will not go out of `0.0.x` before implementing them and a proper testing.
> Future [0.0.x releases may include API breaking changes](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements).

### Examples
```rust
use tchannel_rs::{Config, TChannel, TResult};
use tchannel_rs::handler::{HandlerResult, RequestHandler};
use tchannel_rs::messages::{MessageChannel, RawMessage};

#[tokio::main]
async fn main() -> TResult<()> {
    // Server
    let mut tserver = TChannel::new(Config::default())?;
    let subchannel = tserver.subchannel("service").await?;
    subchannel.register("endpoint", Handler {}).await?;
    tserver.start_server()?;

    // Client
    let tclient = TChannel::new(Config::default())?;
    let subchannel = tclient.subchannel("service").await?;
    let request = RawMessage::new("endpoint".into(), "header".into(), "req body".into());
    let response = subchannel.send(request, "127.0.0.1:8888").await.unwrap();

    // Server shutdown
    tserver.shutdown_server();

    assert_eq!("header", response.header());
    assert_eq!("res body".as_bytes(), response.body().as_ref());
    Ok(())
}

#[derive(Debug)]
struct Handler {}
impl RequestHandler for Handler {
    type REQ = RawMessage;
    type RES = RawMessage;
    fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
        Ok(RawMessage::new(request.endpoint().clone(), request.header().clone(), "res body".into()))
    }
}
```

To run above example following dependencies are required:
```toml
tchannel_rs = *
tokio =  { version = "^1", features = ["macros"] }
env_logger = "^0" # log impl to print tchannel_rs logs
```

## Build

### Examples Subproject

Sample server:
```shell
RUST_LOG=DEBUG cargo run --example server
```

Sample client:
```shell
RUST_LOG=DEBUG cargo run --example client
```

Sample `tchannel-java` server (to check Rust client compatibility):
```shell
# with local Maven/JDK
mvn -f examples-jvm-server package exec:exec -Pserver
# or with Docker
docker-compose --project-directory examples-jvm-server up
# or with Podman (no podman-compose because of network issues)
podman build --file examples-jvm-server/Dockerfile
podman run -p 8888:8888 localhost/examples-jvm-server_tchannel-jvm-server
```

### Update of README.md
```shell
cargo install cargo-readme
cargo readme > README.md
```

---

License: MIT
