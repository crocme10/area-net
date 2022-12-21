//! Frame Codec
use bytes::{Buf, BytesMut};
use std::fmt;
use std::io::Cursor;
use tokio_util::codec::{Decoder, Encoder};

use crate::Frame;

/// codec
#[derive(Debug)]
pub struct FrameCodec;

/// Error type for the codec
#[derive(Debug)]
pub enum Error {
    /// Something is missing
    Incomplete {
        /// Error detail
        detail: String,
    },
    /// Unexpected frame
    InvalidFrame {
        /// Error source
        source: crate::frame::Error,
        /// Error detail
        detail: String,
    },
    /// Unexpected byte sequence
    UnexpectedBytes {
        /// Error detail
        detail: String,
    },
    /// Io Error
    IoError {
        /// Error source
        source: std::io::Error,
    },
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = Error;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        let mut buf = Cursor::new(&src[..]);
        if !buf.has_remaining() {
            Ok(None)
        } else {
            match Frame::check(&mut buf) {
                Ok(_) => {
                    let len = buf.position() as usize;
                    buf.set_position(0);
                    let frame = Frame::parse(&mut buf)?;
                    src.advance(len);
                    Ok(Some(frame))
                }
                Err(err) => Err(err.into()),
            }
        }
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = Error;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        frame.write(dst)?;
        Ok(())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Incomplete { detail } => write!(f, "Incomplete Frame: {}", detail),
            Error::InvalidFrame { source, detail } => {
                write!(f, "Invalid Frame: {} >> {}", detail, source)
            }
            Error::IoError { source } => write!(f, "Frame IO Error: {}", source),
            Error::UnexpectedBytes { detail } => write!(f, "Invalid Frame Content: {}", detail),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::IoError { source }
    }
}

impl From<crate::frame::Error> for Error {
    fn from(err: crate::frame::Error) -> Self {
        Error::InvalidFrame {
            source: err,
            detail: String::from(""),
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn decoder_on_simple_frames() {}

    #[tokio::test]
    async fn decoder_on_array_frame() {}
}
