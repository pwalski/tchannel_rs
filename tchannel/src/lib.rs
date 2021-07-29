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
//!  * [ ] Checksummed frames (only None),
//!  * [ ] Transport of arbitrary payloads (at the moment only Raw payloads),
//!     * [ ] Thrift (WIP)
//!     * [ ] SThrift (streaming Thrift)
//!     * [ ] JSON
//!     * [ ] HTTP
//!     * [x] Raw
//!
//!
//! ## Examples
//! ```
//! use std::net::SocketAddr;
//! use std::str::FromStr;
//! use tchannel::*;
//! use tokio::runtime::Runtime;//!
//!
//! let request = RawMessage::new("endpoint_name".into(), "header".into(), "payload".into());
//! let host = SocketAddr::from_str("host_address:port")?;
//!
//! Runtime::new().unwrap().spawn(async {
//!     let mut tchannel = TChannel::new(ConnectionOptions::default())?;
//!     let subchannel = tchannel.subchannel("server".to_owned()).await?;
//!     match subchannel.send(request, host).await {
//!         Ok(response) => debug!("Response: {:?}", response),
//!         Err(error) => debug!("Fail: {:?}", error),
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
