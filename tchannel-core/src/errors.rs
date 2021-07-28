use std::string::FromUtf8Error;

use crate::frames::payloads::ErrorMsg;
use crate::frames::{TFrame, TFrameId, Type};
use bb8::RunError;
use std::fmt::{Display, Formatter};
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

#[derive(Error, Debug, PartialEq)]
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

#[derive(Error, Debug, PartialEq)]
pub enum CodecError {
    #[error("Codec error: {0}")]
    Error(String),

    /// Represents all cases of `std::io::Error`.
    #[error(transparent)]
    IoError(#[from] IoError),

    #[error(transparent)]
    FormattingError(#[from] core::fmt::Error),

    #[error(transparent)]
    StringDecodingError(#[from] FromUtf8Error),
}

#[derive(Error, Debug, PartialEq)]
pub enum ConnectionError {
    /// Represents general error.
    #[error("Connection error: {0}")]
    Error(String),

    /// Represents all cases of `std::io::Error`.
    #[error(transparent)]
    IoError(#[from] IoError),

    /// Frames codec related error.
    #[error(transparent)]
    FrameError(#[from] CodecError),

    #[error(transparent)]
    SendError(#[from] SendError),

    #[error("Error message: {0:?}")]
    MessageError(ErrorMsg),

    #[error("Unexpected response: {0:?}")]
    UnexpectedResponseError(Type),
}

#[derive(Error, Debug)]
pub struct SendError(tokio::sync::mpsc::error::SendError<TFrameId>);

impl PartialEq for SendError {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

impl Display for SendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

#[derive(Error, Debug)]
pub struct IoError(std::io::Error);

impl PartialEq for IoError {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

impl Display for IoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

impl From<std::io::Error> for CodecError {
    fn from(err: std::io::Error) -> Self {
        CodecError::IoError(IoError(err))
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(err: std::io::Error) -> Self {
        ConnectionError::IoError(IoError(err))
    }
}

impl From<tokio::sync::mpsc::error::SendError<TFrameId>> for ConnectionError {
    fn from(err: tokio::sync::mpsc::error::SendError<TFrameId>) -> Self {
        ConnectionError::SendError(SendError(err))
    }
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
