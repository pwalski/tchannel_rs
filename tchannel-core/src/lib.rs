extern crate core;
extern crate num;

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

pub mod channel;
pub mod handlers;

use crate::channel::frames::payloads::ErrorMsg;
use crate::channel::frames::{TFrame, TFrameId, Type};
use crate::channel::messages::Response;
use crate::TChannelError::{FrameCodecError, FrameError};
use bb8::RunError;
use std::convert::TryFrom;
use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;

#[derive(Error, Debug)]
pub enum TChannelError {
    /// Represents general error.
    #[error("Error: {0}")]
    Error(String),

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
    ReceiveError {
        #[from]
        source: RecvError,
    },

    #[error("Send error")]
    SendError {
        #[from]
        source: SendError<TFrameId>,
    },

    #[error("Frame handling error")]
    FrameError(TFrame),

    #[error("Response error: {0:?}")]
    ResponseError(ErrorMsg),

    #[error("Unexpected response: {0:?}")]
    UnexpectedResponseError(Type),
    // #[error("Error response: {0:?}")]
    // ErrorResponse(Box<Response>),
}

impl From<String> for TChannelError {
    fn from(err: String) -> Self {
        FrameCodecError(err)
    }
}

impl From<RunError<TChannelError>> for TChannelError {
    fn from(err: RunError<TChannelError>) -> Self {
        TChannelError::Error(format!("RunError: {}", err))
    }
}

impl From<TFrame> for TChannelError {
    fn from(frame: TFrame) -> Self {
        FrameError(frame)
    }
}

// impl From<<TryFrom<usize, Error = Type>>::Error> for TChannelError {
//     fn from(err: <TryFrom<usize, Error = Type>>::Error) -> Self {
//         TChannelError::TryFromError(err)
//     }
// }

pub type Error = Box<dyn std::error::Error + Send + Sync>;
