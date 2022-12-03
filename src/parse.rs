//! Provides a type for parsing frames into commands.

use std::fmt;
use std::vec;

use crate::Frame;

/// A frame in the kv protocol
#[derive(Debug)]
pub struct Parse {
    parts: vec::IntoIter<Frame>,
}

/// Error type for the frame parser
#[derive(Debug)]
pub enum Error {
    /// Something is missing
    EndOfStream,
    /// Did not expect additional frame
    UnexpectedFrame,
    /// Unexpected frame type
    InvalidFrameType {
        /// Error detail
        detail: String,
    },
}

impl Parse {
    /// Creates a new parse to parse the frame, which is expected to be an array.
    pub fn new(frame: Frame) -> Result<Parse, Error> {
        let array = match frame {
            Frame::Array(array) => array,
            frame => {
                return Err(Error::InvalidFrameType {
                    detail: format!("Expected Array Frame, got {frame:?}"),
                })
            }
        };
        Ok(Parse {
            parts: array.into_iter(),
        })
    }

    /// Return the next frame, or Error::EndOfStream if there is none
    pub fn next_frame(&mut self) -> Result<Frame, Error> {
        self.parts.next().ok_or(Error::EndOfStream)
    }

    /// Return the string contained in the Frame::Simple
    pub fn next_string(&mut self) -> Result<String, Error> {
        match self.next_frame()? {
            Frame::Simple(s) => Ok(s),
            frame => Err(Error::InvalidFrameType {
                detail: format!("Expected Simple Frame, got {frame:?}"),
            }),
        }
    }

    /// Return the integer contained in the Frame::Timestamp
    pub fn next_integer(&mut self) -> Result<i64, Error> {
        match self.next_frame()? {
            Frame::Timestamp(i) => Ok(i),
            frame => Err(Error::InvalidFrameType {
                detail: format!("Expected Timestamp Frame, got {frame:?}"),
            }),
        }
    }

    /// Return the unsigned contained in the Frame::Integer
    pub fn next_unsigned(&mut self) -> Result<u64, Error> {
        match self.next_frame()? {
            Frame::Integer(u) => Ok(u),
            frame => Err(Error::InvalidFrameType {
                detail: format!("Expected Integer Frame, got {frame:?}"),
            }),
        }
    }

    /// Return Ok(()) if there is no more frames
    pub fn finish(&mut self) -> Result<(), Error> {
        if self.parts.next().is_none() {
            Ok(())
        } else {
            Err(Error::UnexpectedFrame)
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EndOfStream => write!(f, "Unexpected End Of Stream"),
            Error::UnexpectedFrame => write!(f, "Unexpected Frame"),
            Error::InvalidFrameType { detail } => write!(f, "Invalid Frame Type: {}", detail),
        }
    }
}
