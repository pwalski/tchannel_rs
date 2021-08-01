//! TChannel is a network multiplexing and framing RPC protocol created by Uber ([protocol specs](https://github.com/uber/tchannel/blob/master/docs/protocol.md)).
//!
//! ### Disclaimer
//!
//! The project serves as an excuse to learn Rust therefore the implementation may be suboptimal and features are not tested properly.
//!
//! ## Overview
//!
//! Features of TChannel protocol implemented so far:
//!
//!  * [x] A request/response model,
//!  * [x] Multiplexing multiple requests across the same TCP socket,
//!  * [x] Out-of-order responses,
//!  * [ ] Streaming requests and responses,
//!  * [ ] Checksums of frame args (only None),
//!  * [ ] Transport of arbitrary payloads:
//!     * [ ] Thrift
//!     * [ ] SThrift (streaming Thrift)
//!     * [ ] JSON
//!     * [ ] HTTP
//!     * [x] Raw
//!
//! Additional nonfunctional TODOs:
//!
//!  * [ ] Proper tests (right now only few happy paths)
//!  * [ ] Investigate WASI support
//!
//! ## Examples
//! ```
//! use std::net::SocketAddr;
//! use std::str::FromStr;
//! use tchannel_protocol::messages::raw::RawMessage;
//! use tchannel_protocol::messages::MessageChannel;
//! use tchannel_protocol::{TChannel,ConnectionOptions};
//! use tokio::runtime::Runtime;
//!
//! Runtime::new().unwrap().spawn(async {
//!     let request = RawMessage::new("endpoint_name".into(), "header".into(), "payload".into());
//!     let host = SocketAddr::from_str("host_address:port").unwrap();
//!     let mut tchannel = TChannel::new(ConnectionOptions::default()).unwrap();
//!     let subchannel = tchannel.subchannel("server".to_owned()).await.unwrap();
//!     match subchannel.send(request, host).await {
//!         Ok(response) => println!("Response: {:?}", response),
//!         Err(error) => println!("Fail: {:?}", error),
//!     }
//! });
//! ```
//!

#[macro_use]
extern crate getset;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate derive_builder;

#[macro_use]
extern crate derive_new;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

pub(crate) mod channel;
pub(crate) mod connection;
pub(crate) mod defragmentation;
pub(crate) mod fragmentation;
pub(crate) mod frames;
pub(crate) mod handler;

pub mod errors;
pub mod messages;

pub use self::channel::SubChannel;
pub use self::channel::TChannel;
pub use self::connection::ConnectionOptions;
pub use self::connection::ConnectionOptionsBuilder;
