use crate::frames::payloads::{ErrorCode, ErrorMsg, Tracing};
use crate::frames::{TFrameId, Type};
use crate::messages::Message;
use bb8::RunError;

use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;
use strum::ParseError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

/// General TChannel error.
#[derive(Error, Debug, PartialEq)]
pub enum TChannelError {
    #[error("TChannel error: {0}")]
    Error(String),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    ConnectionPoolError(#[from] RunError<ConnectionError>),
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

    // #[error("Error message: {0:?}")]
    // MessageError(ErrorMsg),
    #[error("Error message: {0:?}")]
    MessageErrorId(ErrorMsg, u32),

    #[error("Unexpected response: {0:?}")]
    UnexpectedResponseError(Type),
}

/// Request handler Error
#[derive(Error, Debug, PartialEq)]
pub enum HandlerError<RES: Message> {
    #[error(transparent)]
    InternalError(#[from] TChannelError),

    /// A general error.
    #[error("Handler error: {0}")]
    GeneralError(String),

    /// A message response with error code.
    #[error("Handler registration error: {0}")]
    MessageError(RES),
}

/// Newtype of [`std::io::Error`].
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

#[cfg(feature = "json")]
impl From<serde_json::Error> for CodecError {
    fn from(err: serde_json::Error) -> Self {
        CodecError::Error(err.to_string())
    }
}

impl From<SendError<TFrameId>> for ConnectionError {
    fn from(err: SendError<TFrameId>) -> Self {
        ConnectionError::MessageErrorId(
            ErrorMsg::new(
                ErrorCode::UnexpectedError,
                Tracing::default(),
                format!("Failed to handle frame: {}", err),
            ),
            *err.0.id(),
        )
    }
}

impl From<SendError<TFrameId>> for TChannelError {
    fn from(err: SendError<TFrameId>) -> Self {
        TChannelError::from(ConnectionError::from(err))
    }
}

impl From<(u32, ConnectionError, Tracing)> for ConnectionError {
    fn from((id, err, tracing): (u32, ConnectionError, Tracing)) -> Self {
        match err {
            ConnectionError::MessageErrorId(_, _) => err,
            _ => {
                let msg = ErrorMsg::new(ErrorCode::UnexpectedError, tracing, err.to_string());
                ConnectionError::MessageErrorId(msg, id)
            }
        }
    }
}

impl From<(u32, TChannelError, Tracing)> for TChannelError {
    fn from((id, err, tracing): (u32, TChannelError, Tracing)) -> Self {
        let connection_error = match err {
            TChannelError::ConnectionError(err) => ConnectionError::from((id, err, tracing)),
            err => {
                let error_msg = ErrorMsg::new(ErrorCode::UnexpectedError, tracing, err.to_string());
                ConnectionError::MessageErrorId(error_msg, id)
            }
        };
        TChannelError::ConnectionError(connection_error)
    }
}
