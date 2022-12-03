//! Frame Codec
use bytes::{Buf, BufMut, BytesMut};
use std::fmt;
use std::io::Cursor;
use std::io::Write;
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

    fn encode(&mut self, src: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match src {
            Frame::Array(fs) => {
                dst.reserve(fs.iter().fold(11, |acc, f| acc + frame_len(f))); // '*' + length + frames
                dst.put_u8(b'*');

                // Encode the length of the array of frames
                write_u64(fs.len() as u64, dst)?;

                // Iterate and encode each frame in the array.
                for f in fs {
                    write_frame(&f, dst)?;
                }
            }
            // The frame type is a literal. Encode the value directly.
            f => {
                write_frame(&f, dst)?;
            }
        }

        Ok(())
    }
}

/// Return the length of the frame
fn frame_len(frame: &Frame) -> usize {
    match frame {
        Frame::Simple(val) => 3 + val.as_bytes().len(),
        Frame::Error(val) => 3 + val.as_bytes().len(),
        Frame::Integer(_) => 11,
        Frame::Timestamp(_) => 11,
        Frame::Null => 5,
        Frame::Bulk(val) => 11 + val.len(),
        Frame::Array(frames) => frames.iter().fold(11, |acc, f| acc + frame_len(f)),
    }
}

/// Write a frame literal to the file
fn write_frame(frame: &Frame, dst: &mut BytesMut) -> Result<(), Error> {
    match frame {
        Frame::Simple(val) => {
            let bytes = val.as_bytes();
            dst.reserve(1 + bytes.len() + 2); // '+' + bytes + '\r' + '\n'
            dst.put_u8(b'+');
            dst.extend_from_slice(bytes);
            dst.put(&b"\r\n"[..]);
        }
        Frame::Error(val) => {
            let bytes = val.as_bytes();
            dst.reserve(1 + bytes.len() + 2);
            dst.put_u8(b'-');
            dst.put(bytes);
            dst.put(&b"\r\n"[..]);
        }
        Frame::Integer(val) => {
            dst.reserve(11); // ':' + u64 + '\r' + '\n'
            dst.put_u8(b':');
            write_u64(*val, dst)?;
        }
        Frame::Timestamp(val) => {
            dst.reserve(11); // ':' + u64 + '\r' + '\n'
            dst.put_u8(b'@');
            write_i64(*val, dst)?;
        }
        Frame::Null => {
            dst.reserve(5);
            dst.put(&b"$-1\r\n"[..]);
        }
        Frame::Bulk(val) => {
            dst.reserve(11 + val.len()); // '$' + u64 + bytes + '\r' + '\n'
            dst.put_u8(b'$');
            write_u64(val.len() as u64, dst)?;
            dst.extend_from_slice(val);
            dst.put(&b"\r\n"[..]);
        }
        // Encoding an `Array` from within a value cannot be done using a
        // recursive strategy. In general, async fns do not support
        // recursion. Mini-redis has not needed to encode nested arrays yet,
        // so for now it is skipped.
        Frame::Array(_val) => unreachable!(),
    }

    Ok(())
}

fn write_u64(val: u64, dst: &mut BytesMut) -> Result<(), Error> {
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;
    let pos = buf.position() as usize;
    dst.reserve(2 + pos);
    dst.extend_from_slice(&buf.get_ref()[..pos]);
    dst.put(&b"\r\n"[..]);
    Ok(())
}

// TODO need to write generic code
fn write_i64(val: i64, dst: &mut BytesMut) -> Result<(), Error> {
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;
    let pos = buf.position() as usize;
    dst.reserve(2 + pos);
    dst.extend_from_slice(&buf.get_ref()[..pos]);
    dst.put(&b"\r\n"[..]);
    Ok(())
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
