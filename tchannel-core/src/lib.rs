extern crate num;

#[macro_use]
extern crate getset;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate derive_builder;

#[macro_use]
extern crate bitflags;

pub mod channel;
pub mod handlers;

use crate::TChannelError::FrameCodecError;
use std::string::FromUtf8Error;
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

    #[error("Frame codec error: {0}")]
    FrameCodecError(String),

    #[error("Formatting error")]
    FormattingError(#[from] core::fmt::Error),

    #[error("String decoding error")]
    StringDecodingError(#[from] FromUtf8Error),
}

impl From<String> for TChannelError {
    fn from(err: String) -> Self {
        FrameCodecError(err)
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
