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
//!  * [ ] Request response TTL
//!  * [ ] Canceling request
//!
//! ## Examples
//! ```
//! use tchannel_protocol::errors::TChannelError;
//! use tchannel_protocol::handler::{RequestHandler, Response};
//! use tchannel_protocol::messages::raw::RawMessage;
//! use tchannel_protocol::messages::MessageChannel;
//! use tchannel_protocol::{Config, TChannel};
//! use tokio::runtime::Runtime;
//!
//! fn main() -> Result<(), TChannelError> {
//!     let tserver = Runtime::new().unwrap().block_on(run())?;
//!     // Shutdown outside of async
//!     Ok(tserver.shutdown_server())
//! }
//!
//! async fn run() -> Result<TChannel, TChannelError> {
//!     // Server
//!     let mut tserver = TChannel::new(Config::default())?;
//!     let subchannel = tserver.subchannel("service".to_string()).await?;
//!     subchannel.register("endpoint", Handler {}).await?;
//!     tserver.start_server()?;
//!
//!     // Client
//!     let tclient = TChannel::new(Config::default())?;
//!     let subchannel = tclient.subchannel("service".to_string()).await?;
//!     let request = RawMessage::new("endpoint".into(), "a".into(), "b".into());
//!     let response_res = subchannel.send(request, "127.0.0.1:8888").await;
//!
//!     assert!(response_res.is_ok());
//!     let response = response_res.unwrap();
//!     assert_eq!("a", response.header());
//!     assert_eq!("y".as_bytes(), response.body().as_ref());
//!     Ok(tserver)
//! }
//!
//! #[derive(Debug)]
//! struct Handler {}
//! impl RequestHandler for Handler {
//!     type REQ = RawMessage;
//!     type RES = RawMessage;
//!     fn handle(&mut self, request: Self::REQ) -> Response<Self::RES> {
//!         let req_header = request.header().clone();
//!         Ok(RawMessage::new("x".into(), req_header, "y".into()))
//!     }
//! }
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
pub(crate) mod server;
pub(crate) mod subchannel;

pub mod errors;
pub mod handler;
pub mod messages;

pub use self::channel::TChannel;
pub use self::connection::Config;
pub use self::subchannel::SubChannel;
