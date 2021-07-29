# tchannel

TChannel is a network multiplexing and framing RPC protocol created by Uber ([protocol specs](https://github.com/uber/tchannel/blob/master/docs/protocol.md)).

### Overview

Features of TChannel protocol implemented so far:

 * [x] A request/response model,
 * [x] Multiplexing multiple requests across the same TCP socket,
 * [x] Out-of-order responses,
 * [ ] Streaming requests and responses,
 * [ ] Checksummed frames (only None),
 * [ ] Transport of arbitrary payloads (at the moment only Raw payloads),
    * [ ] Thrift (WIP)
    * [ ] SThrift (streaming Thrift)
    * [ ] JSON
    * [ ] HTTP
    * [x] Raw


### Examples
```rust
use std::net::SocketAddr;
use std::str::FromStr;
use tchannel::*;
use tokio::runtime::Runtime;//!

let request = RawMessage::new("endpoint_name".into(), "header".into(), "payload".into());
let host = SocketAddr::from_str("host_address:port")?;

Runtime::new().unwrap().spawn(async {
    let mut tchannel = TChannel::new(ConnectionOptions::default())?;
    let subchannel = tchannel.subchannel("server".to_owned()).await?;
    match subchannel.send(request, host).await {
        Ok(response) => debug!("Response: {:?}", response),
        Err(error) => debug!("Fail: {:?}", error),
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
