//! TChannel is a network multiplexing and framing RPC protocol created by Uber ([protocol specs](https://github.com/uber/tchannel/blob/master/docs/protocol.md)).
//!
//! ## Overview
//!
//! Features of TChannel protocol implemented so far:
//!
//!  * [x] A request/response model,
//!  * [x] Multiplexing multiple requests across the same TCP socket,
//!  * [x] Out-of-order responses,
//!  * [ ] Streaming requests and responses,
//!  * [ ] Checksums of frame args (only _None_),
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
//!  * [ ] Use Tower?
//!  * [ ] Implement Serde Serialize/Deserialize to Message types
//!
//! The goal of the project is to provide a similar API to Java TChannel implementation which is why both connection pools and server task handler are hidden from user.
//!
//! **Disclaimer**
//!
//! > The project was used to learn Rust and it still has some missing features, so it will not go out of _alpha_ before implementing them and a proper testing.
//! > The API may be a subject of change in consecutive `0.1.0-alpha.X` releases.
//!
//! ## Examples
//! ```
//! use tchannel_protocol::{Config, TChannel, TResult};
//! use tchannel_protocol::handler::{HandlerResult, RequestHandler};
//! use tchannel_protocol::messages::{MessageChannel, RawMessage};
//!
//! #[tokio::main]
//! async fn main() -> TResult<()> {
//!     // Server
//!     let mut tserver = TChannel::new(Config::default())?;
//!     let subchannel = tserver.subchannel("service").await?;
//!     subchannel.register("endpoint", Handler {}).await?;
//!     tserver.start_server()?;
//!
//!     // Client
//!     let tclient = TChannel::new(Config::default())?;
//!     let subchannel = tclient.subchannel("service").await?;
//!     let request = RawMessage::new("endpoint".into(), "header".into(), "req body".into());
//!     let response = subchannel.send(request, "127.0.0.1:8888").await.unwrap();
//!
//!     // Server shutdown
//!     tserver.shutdown_server();
//!
//!     assert_eq!("header", response.header());
//!     assert_eq!("res body".as_bytes(), response.body().as_ref());
//!     Ok(())
//! }
//!
//! #[derive(Debug)]
//! struct Handler {}
//! impl RequestHandler for Handler {
//!     type REQ = RawMessage;
//!     type RES = RawMessage;
//!     fn handle(&mut self, request: Self::REQ) -> HandlerResult<Self::RES> {
//!         Ok(RawMessage::new(request.endpoint().clone(), request.header().clone(), "res body".into()))
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
pub(crate) mod config;
pub(crate) mod connection;
pub(crate) mod defragmentation;
pub(crate) mod fragmentation;
pub(crate) mod frames;
pub(crate) mod server;
pub(crate) mod subchannel;

/// TChannel errors.
pub mod errors;
/// Handlers registered in [`SubChannel`](crate::SubChannel) and called by server started from [`TChannel`](crate::TChannel).
pub mod handler;
/// Messages send from [`SubChannel`](crate::SubChannel) and handled by [`RequestHandler`](crate::handler::RequestHandler) ([`RequestHandlerAsync`](crate::handler::RequestHandlerAsync))
pub mod messages;

pub use self::channel::TChannel;
pub use self::channel::TResult;
pub use self::subchannel::SubChannel;
pub use config::Config;
pub use config::ConfigBuilder;
