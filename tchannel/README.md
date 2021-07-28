# tchannel

## TChannel protocol

Implementation of Uber's [TChannel](https://github.com/uber/tchannel) network multiplexing (NYI) and framing RPC protocol.
Library allows to implement both client and server (NYI).

## Examples
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
    }//!
});
```

Current version: 0.1.0

License: MIT OR Apache-2.0
