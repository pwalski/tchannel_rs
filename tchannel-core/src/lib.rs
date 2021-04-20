extern crate num;

#[macro_use]
extern crate getset;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate derive_builder;

pub mod channel;
pub mod connection;
pub mod frame;
pub mod handlers;
pub mod transport;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TChannelError {
    /// Represents general error.
    #[error("Error")]
    Error,

    /// Represents connection error.
    #[error("Read error")]
    ConnectionError { source: std::io::Error },

    /// Represents all other cases of `std::io::Error`.
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error("Frame parsing error: {0}")]
    FrameParsingError(String),
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
