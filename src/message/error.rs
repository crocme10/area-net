//! Error type common to all messages

use std::fmt;

/// Error type for messages
#[derive(Debug)]
pub enum Error {
    /// Parse Error
    Parse {
        /// source field
        source: crate::parse::Error,
    },

    /// Frame Error
    Frame {
        /// source field
        source: crate::frame::Error,
    },

    /// Unexpected Message
    UnexpectedMessage {
        /// details about the unexpected message.
        detail: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { source } => write!(f, "Parsing Error: {source}"),
            Error::Frame { source } => write!(f, "Framing Error: {source}"),
            Error::UnexpectedMessage { detail } => write!(f, "Unexpected Message {detail}"),
        }
    }
}

impl From<crate::parse::Error> for Error {
    fn from(err: crate::parse::Error) -> Self {
        Error::Parse { source: err }
    }
}

impl From<crate::frame::Error> for Error {
    fn from(err: crate::frame::Error) -> Self {
        Error::Frame { source: err }
    }
}
