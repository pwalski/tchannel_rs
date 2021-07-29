![ci](https://github.com/pwalski/tchannel-rust/actions/workflows/ci.yml/badge.svg)

# tchannel

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

Additional nonfunctional TODOs:

 * [ ] Proper tests (right now only few happy paths)
 * [ ] Investigate WASI support

### Examples
```rust
use std::net::SocketAddr;
use std::str::FromStr;
use tchannel::messages::raw::RawMessage;
use tchannel::messages::MessageChannel;
use tchannel::{TChannel,ConnectionOptions};
use tokio::runtime::Runtime;

Runtime::new().unwrap().spawn(async {
    let request = RawMessage::new("endpoint_name".into(), "header".into(), "payload".into());
    let host = SocketAddr::from_str("host_address:port").unwrap();
    let mut tchannel = TChannel::new(ConnectionOptions::default()).unwrap();
    let subchannel = tchannel.subchannel("server".to_owned()).await.unwrap();
    match subchannel.send(request, host).await {
        Ok(response) => println!("Response: {:?}", response),
        Err(error) => println!("Fail: {:?}", error),
    }
});
```


## Build

Update of README
```shell
cargo install cargo-readme
cargo readme -r tchannel -t ../README.tpl > README.md
```

## Examples Subproject

Sample `tchannel-java` server:
```shell
mvn -f tchannel-examples/server package exec:exec -Pserver
```

Basic client scenario:
```shell
RUST_LOG=DEBUG cargo run -p tchannel-examples --example basic
```

---

License: MIT OR Apache-2.0
