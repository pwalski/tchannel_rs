use std::string::FromUtf8Error;

use crate::channel::frames::payloads::ErrorMsg;
use crate::channel::frames::{TFrame, TFrameId, Type};
use bb8::RunError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;

#[derive(Error, Debug)]
pub enum TChannelError {
    /// Represents general error.
    #[error("TChannel error: {0}")]
    Error(String),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    ConnectionPoolError(#[from] RunError<ConnectionError>),
}

#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Codec error: {0}")]
    Error(String),

    /// Represents all cases of `std::io::Error`.
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    FormattingError(#[from] core::fmt::Error),

    #[error(transparent)]
    StringDecodingError(#[from] FromUtf8Error),
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    /// Represents general error.
    #[error("Connection error: {0}")]
    Error(String),

    /// Represents all cases of `std::io::Error`.
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// Frames codec related error.
    #[error(transparent)]
    FrameError(#[from] CodecError),

    #[error(transparent)]
    SendError(#[from] SendError<TFrameId>),

    #[error("Error message: {0:?}")]
    MessageError(ErrorMsg),

    #[error("Unexpected response: {0:?}")]
    UnexpectedResponseError(Type),
}

impl From<String> for TChannelError {
    fn from(err: String) -> Self {
        TChannelError::Error(err)
    }
}

impl From<String> for ConnectionError {
    fn from(err: String) -> Self {
        ConnectionError::Error(err)
    }
}

impl From<String> for CodecError {
    fn from(err: String) -> Self {
        CodecError::Error(err)
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
