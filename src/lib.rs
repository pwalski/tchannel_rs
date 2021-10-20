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
//!  * [ ] Routing mesh
//!  * [ ] Tracing
//!
//! Other TODOs:
//!
//!  * [ ] Proper tests (right now only few happy paths)
//!  * [ ] Request response TTL
//!  * [ ] Cancel request
//!  * [ ] Claim requests
//!
//! ## Examples
//! ```
//! use tchannel_protocol::{Config, TChannel, TResult};
//! use tchannel_protocol::handler::{HandlerResult, RequestHandler};
//! use tchannel_protocol::messages::MessageChannel;
//! use tchannel_protocol::messages::raw::RawMessage;
//! use tokio::runtime::Runtime;
//!
//! #[tokio::main]
//! async fn main() -> TResult<()> {
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
//!     // Server shutdown
//!     tserver.shutdown_server();
//!
//!     assert!(response_res.is_ok());
//!     let response = response_res.unwrap();
//!     assert_eq!("a", response.header());
//!     assert_eq!("y".as_bytes(), response.body().as_ref());
//!     Ok(())
//! }
//!
//! #[derive(Debug)]
//! struct Handler {}
//! impl RequestHandler for Handler {
//!     type REQ = RawMessage;
//!     type RES = RawMessage;
//!     fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
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
pub use self::channel::TResult;
pub use self::connection::Config;
pub use self::subchannel::SubChannel;
