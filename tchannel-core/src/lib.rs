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

use crate::TChannelError::{ConnectionError, FrameCodecError};
use bb8::RunError;
use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

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

    #[error("Timeout error")]
    TimeoutError,

    #[error("Receive error")]
    ReceiveError(#[from] RecvError),
}

impl From<String> for TChannelError {
    fn from(err: String) -> Self {
        FrameCodecError(err)
    }
}

impl From<RunError<TChannelError>> for TChannelError {
    fn from(err: RunError<TChannelError>) -> Self {
        TChannelError::Error
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
