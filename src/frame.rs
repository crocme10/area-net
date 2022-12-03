//! Provides a type representing a Redis protocol frame as well as utilities for
//! parsing frames from a byte array.
//!
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::convert::TryInto;
use std::fmt;
use std::io::Cursor;
use std::num::TryFromIntError;
use tokio::io::AsyncWriteExt;

/// A frame in the kv protocol
#[derive(Clone, Debug)]
pub enum Frame {
    /// Just a string
    Simple(String),
    /// An error
    Error(String),
    /// An integer
    // FIXME Naming of frames should probably
    // be something like Int and UInt
    Integer(u64),
    /// Timestamp (μs)
    Timestamp(i64),
    /// Raw bytes
    Bulk(Bytes),
    /// Empty frame
    Null,
    /// Multiple frames
    Array(Vec<Frame>),
}

/// Error type for frames
#[derive(Debug)]
pub enum Error {
    /// Something is missing
    Incomplete {
        /// Error detail
        detail: String,
    },
    /// Unexpected frame type
    InvalidFrameType {
        /// Error detail
        detail: String,
    },
    /// Unexpected byte sequence
    UnexpectedBytes {
        /// Error detail
        detail: String,
    },
    /// Invalid Numerical Value
    InvalidNumeric {
        /// Error detail
        detail: String,
    },
    /// IO Error
    IoError {
        /// Error detail
        detail: String,
    },
}

impl Frame {
    /// Returns an empty array
    pub(crate) fn array() -> Frame {
        Frame::Array(vec![])
    }

    /// push simple
    pub(crate) fn push_simple(&mut self, s: String) -> Result<(), Error> {
        match self {
            Frame::Array(vec) => {
                vec.push(Frame::Simple(s));
                Ok(())
            }
            _ => Err(Error::InvalidFrameType {
                detail: String::from("Expected Frame Type Array"),
            }),
        }
    }

    /// push integer
    pub(crate) fn push_integer(&mut self, i: i64) -> Result<(), Error> {
        match self {
            Frame::Array(vec) => {
                vec.push(Frame::Timestamp(i));
                Ok(())
            }
            _ => Err(Error::InvalidFrameType {
                detail: String::from("Expected Frame Type Array"),
            }),
        }
    }

    /// push integer
    pub(crate) fn push_unsigned(&mut self, u: u64) -> Result<(), Error> {
        match self {
            Frame::Array(vec) => {
                vec.push(Frame::Integer(u));
                Ok(())
            }
            _ => Err(Error::InvalidFrameType {
                detail: String::from("Expected Frame Type Array"),
            }),
        }
    }

    /// Checks if an entire message can be decoded from `src`
    pub fn check(src: &mut Cursor<&[u8]>) -> Result<(), Error> {
        // We already checked src.has_remaining(), so the get_u8() won't panic.
        match src.get_u8() {
            b'+' => {
                get_line(src)?;
                Ok(())
            }
            b'-' => {
                get_line(src)?;
                Ok(())
            }
            b':' => {
                get_unsigned(src)?;
                Ok(())
            }
            b'@' => {
                get_integer(src)?;
                Ok(())
            }
            b'*' => {
                let len = get_unsigned(src)?;
                for _ in 0..len {
                    Frame::check(src)?;
                }
                Ok(())
            }
            actual => Err(Error::InvalidFrameType {
                detail: format!("Unexpected frame id: {}", actual),
            }),
        }
    }

    /// The message has already been validated with check
    pub fn parse(src: &mut Cursor<&[u8]>) -> Result<Frame, Error> {
        match src.get_u8() {
            b'+' => {
                let line = get_line(src)?;
                let string =
                    String::from_utf8(line[..].to_vec()).map_err(|err| Error::UnexpectedBytes {
                        detail: format!("Invalid UTF8: {err}"),
                    })?;
                Ok(Frame::Simple(string))
            }
            b'-' => {
                let line = get_line(src)?;
                let string =
                    String::from_utf8(line[..].to_vec()).map_err(|err| Error::UnexpectedBytes {
                        detail: format!("Invalid UTF8: {err}"),
                    })?;
                Ok(Frame::Error(string))
            }
            b':' => {
                let len = get_unsigned(src)?;
                Ok(Frame::Integer(len))
            }
            b'@' => {
                let ts = get_integer(src)?;
                Ok(Frame::Timestamp(ts))
            }
            b'*' => {
                let len: usize = get_unsigned(src)?.try_into()?;
                let mut frames = Vec::with_capacity(len);
                for _ in 0..len {
                    frames.push(Frame::parse(src)?);
                }
                Ok(Frame::Array(frames))
            }
            _ => unimplemented!(),
        }
    }

    /// Documentation
    pub async fn write<T: AsyncWriteExt>(&self, dst: &mut T) -> Result<(), Error>
    where
        T: AsyncWriteExt,
        T: Unpin,
    {
        // Arrays are encoded by encoding each entry. All other frame types are
        // considered literals. For now, mini-redis is not able to encode
        // recursive frame structures. See below for more details.
        match self {
            Frame::Array(val) => {
                // Encode the frame type prefix. For an array, it is `*`.
                dst.write_u8(b'*').await?;

                // Encode the length of the array.
                write_unsigned(dst, val.len() as u64).await?;

                // Iterate and encode each entry in the array.
                for entry in &**val {
                    entry.write_value(dst).await?;
                }
            }
            // The frame type is a literal. Encode the value directly.
            _ => self.write_value(dst).await?,
        }

        // Ensure the encoded frame is written to the socket. The calls above
        // are to the buffered file and writes. Calling `flush` writes the
        // remaining contents of the buffer to the socket.
        dst.flush().await?;
        Ok(())
    }

    /// Write a frame literal to the file
    async fn write_value<T: AsyncWriteExt>(&self, dst: &mut T) -> Result<(), Error>
    where
        T: AsyncWriteExt,
        T: Unpin,
    {
        match self {
            Frame::Simple(val) => {
                dst.write_u8(b'+').await?;
                dst.write_all(val.as_bytes()).await?;
                dst.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                dst.write_u8(b'-').await?;
                dst.write_all(val.as_bytes()).await?;
                dst.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                dst.write_u8(b':').await?;
                write_unsigned(dst, *val).await?;
            }
            Frame::Timestamp(val) => {
                dst.write_u8(b'@').await?;
                write_integer(dst, *val).await?;
            }
            Frame::Null => {
                dst.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();

                dst.write_u8(b'$').await?;
                write_unsigned(dst, len as u64).await?;
                dst.write_all(val).await?;
                dst.write_all(b"\r\n").await?;
            }
            // Encoding an `Array` from within a value cannot be done using a
            // recursive strategy. In general, async fns do not support
            // recursion. Mini-redis has not needed to encode nested arrays yet,
            // so for now it is skipped.
            Frame::Array(_val) => unreachable!(),
        }

        Ok(())
    }

    /// Documentation
    pub async fn write_buf(&self, dst: &mut BytesMut) -> Result<(), Error> {
        // Arrays are encoded by encoding each entry. All other frame types are
        // considered literals. For now, mini-redis is not able to encode
        // recursive frame structures. See below for more details.
        match self {
            Frame::Array(val) => {
                // Encode the frame type prefix. For an array, it is `*`.
                dst.put_u8(b'*');

                // Encode the length of the array.
                write_unsigned_buf(dst, val.len() as u64).await?;

                // Iterate and encode each entry in the array.
                for entry in &**val {
                    entry.write_value_buf(dst).await?;
                }
            }
            // The frame type is a literal. Encode the value directly.
            _ => {
                self.write_value_buf(dst).await?;
            }
        }

        Ok(())
    }

    /// Write a frame literal to the file
    async fn write_value_buf(&self, dst: &mut BytesMut) -> Result<(), Error> {
        match self {
            Frame::Simple(val) => {
                dst.put_u8(b'+');
                unsafe {
                    dst.advance_mut(1);
                };
                dst.put(val.as_bytes());
                unsafe {
                    dst.advance_mut(val.as_bytes().len());
                };
                dst.put(&b"\r\n"[..]);
                unsafe {
                    dst.advance_mut(2);
                };
            }
            Frame::Error(val) => {
                dst.put_u8(b'-');
                dst.put(val.as_bytes());
                dst.put(&b"\r\n"[..]);
            }
            Frame::Integer(val) => {
                dst.put_u8(b':');
                write_unsigned_buf(dst, *val).await?;
            }
            Frame::Timestamp(val) => {
                dst.put_u8(b'@');
                write_integer_buf(dst, *val).await?;
            }
            Frame::Null => {
                dst.put(&b"$-1\r\n"[..]);
            }
            Frame::Bulk(val) => {
                let len = val.len();

                dst.put_u8(b'$');
                write_unsigned_buf(dst, len as u64).await?;
                dst.put(val.as_ref());
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
}

/// Write a unsigned frame to the file
async fn write_unsigned<T>(dst: &mut T, val: u64) -> Result<(), Error>
where
    T: AsyncWriteExt,
    T: Unpin,
{
    use std::io::Write;

    // Convert the value to a string
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;

    let pos = buf.position() as usize;
    dst.write_all(&buf.get_ref()[..pos]).await?;
    dst.write_all(b"\r\n").await?;

    Ok(())
}

/// Write a unsigned frame to the file
async fn write_unsigned_buf(dst: &mut BytesMut, val: u64) -> Result<(), Error> {
    use std::io::Write;

    // Convert the value to a string
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;

    let pos = buf.position() as usize;
    dst.put(&buf.get_ref()[..pos]);
    dst.put(&b"\r\n"[..]);

    Ok(())
}

/// Write an integer frame to the file
async fn write_integer<T>(dst: &mut T, val: i64) -> Result<(), Error>
where
    T: AsyncWriteExt,
    T: Unpin,
{
    use std::io::Write;

    // Convert the value to a string
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;

    let pos = buf.position() as usize;
    dst.write_all(&buf.get_ref()[..pos]).await?;
    dst.write_all(b"\r\n").await?;

    Ok(())
}

/// Write an integer frame to the file
async fn write_integer_buf(dst: &mut BytesMut, val: i64) -> Result<(), Error> {
    use std::io::Write;

    // Convert the value to a string
    let mut buf = [0u8; 20];
    let mut buf = Cursor::new(&mut buf[..]);
    write!(&mut buf, "{}", val)?;

    let pos = buf.position() as usize;
    dst.put(&buf.get_ref()[..pos]);
    dst.put(&b"\r\n"[..]);

    Ok(())
}

// Find a End Of Frame Marker (\r\n), and returns a slice up to that mark
// Change the position of the cursor to point to just after the end of frame.
fn get_line<'a>(src: &mut Cursor<&'a [u8]>) -> Result<&'a [u8], Error> {
    if !src.has_remaining() {
        return Err(Error::Incomplete {
            detail: String::from("get line, buflen < 2"),
        });
    }

    // Scan the bytes directly
    let start = src.position() as usize;
    // Scan to the second to last byte
    let end = src.get_ref().len() - 1;
    for i in start..end {
        if src.get_ref()[i] == b'\r' && src.get_ref()[i + 1] == b'\n' {
            // We found a line, update the position to 1 past the \n
            src.set_position((i + 2) as u64);

            // Return the line
            return Ok(&src.get_ref()[start..i]);
        }
    }
    Err(Error::Incomplete {
        detail: String::from("get line, buflen < 2"),
    })
}

fn get_unsigned(src: &mut Cursor<&[u8]>) -> Result<u64, Error> {
    use atoi::atoi;
    let line = get_line(src)?;
    atoi::<u64>(line).ok_or_else(|| Error::UnexpectedBytes {
        detail: String::from("Invalid unsigned frame"),
    })
}

fn get_integer(src: &mut Cursor<&[u8]>) -> Result<i64, Error> {
    use atoi::atoi;
    let line = get_line(src)?;
    atoi::<i64>(line).ok_or_else(|| Error::UnexpectedBytes {
        detail: String::from("Invalid integer frame"),
    })
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Incomplete { detail } => write!(f, "Incomplete Frame: {}", detail),
            Error::InvalidFrameType { detail } => write!(f, "Invalid Frame Type: {}", detail),
            Error::UnexpectedBytes { detail } => write!(f, "Invalid Frame Content: {}", detail),
            Error::InvalidNumeric { detail } => write!(f, "Invalid Numerical Value: {}", detail),
            Error::IoError { detail } => write!(f, "IO Error: {}", detail),
        }
    }
}

impl From<TryFromIntError> for Error {
    fn from(err: TryFromIntError) -> Self {
        Error::InvalidNumeric {
            detail: format!("Could not convert to numerical value {}", err),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError {
            detail: format!("IO Error: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_line_works() {
        let mut b = Cursor::new(&b"Hello\r\nWorld"[..]);
        let a = get_line(&mut b).unwrap();
        assert_eq!(&b.get_ref()[b.position() as usize..], b"World");
        assert_eq!(&a, b"Hello");
    }

    #[test]
    fn get_line_works_on_empty_buffer() {
        let mut b = Cursor::new(&b""[..]);
        let a = get_line(&mut b);
        assert!(a.is_err());
    }

    #[test]
    fn get_line_works_with_end_of_frame_at_end_of_buffer() {
        let mut b = Cursor::new(&b"Hello World\r\n"[..]);
        let a = get_line(&mut b).unwrap();
        // assert_eq!(b.get_ref(), b"");
        assert_eq!(&b.get_ref()[b.position() as usize..], b"");
        assert_eq!(&a, b"Hello World");
    }

    #[test]
    fn get_line_works_with_just_end_of_frame() {
        let mut b = Cursor::new(&b"\r\n"[..]);
        let a = get_line(&mut b).unwrap();
        assert_eq!(&b.get_ref()[b.position() as usize..], b"");
        assert_eq!(&a, b"");
    }
}
