use crate::frames::payloads::ErrorMsg;
use crate::frames::{TFrameId, Type};
use bb8::RunError;
use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;
use strum::ParseError;
use thiserror::Error;

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

    #[error(transparent)]
    HandlerError(#[from] HandlerError),
}

/// Frame encoding error.
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

    #[error(transparent)]
    ParseError(#[from] ParseError),
}

/// Host connection error.
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

    #[error("Error message: {0:?}")]
    MessageErrorId(ErrorMsg, u32),

    #[error("Unexpected response: {0:?}")]
    UnexpectedResponseError(Type),
}

/// Request handler Error
#[derive(Error, Debug, PartialEq)]
pub enum HandlerError {
    /// Represents general error.
    #[error("Handler error: {0}")]
    Error(String),

    #[error("Handler registration error: {0}")]
    RegistrationError(String),
}

#[derive(Error, Debug)]
pub struct SendError(tokio::sync::mpsc::error::SendError<TFrameId>);

impl PartialEq for SendError {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Display for SendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Error, Debug)]
pub struct IoError(std::io::Error);

impl PartialEq for IoError {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Display for IoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
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
