//! Connection Rejection

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// Get the value of a key
#[derive(Debug)]
pub struct ConnRejection {
    /// Id of the peer issuing a connection rejection
    pub id: String,

    /// Reason for the rejection. This could become an enum,
    /// like 'banned', 'duplicate'
    pub reason: String,
}

impl ConnRejection {
    /// Creates a new message
    pub fn new(id: impl ToString, reason: impl ToString) -> ConnRejection {
        ConnRejection {
            id: id.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Accessor for the id
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Accessor for the reason
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Extract a ConnRejection message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<ConnRejection, Error> {
        let id = parse.next_string()?;
        let reason = parse.next_string()?;
        Ok(ConnRejection { id, reason })
    }

    /// Convert the Connection Rejection into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let mut frame = Frame::array();
        frame.push_simple(String::from("CONN_REJECT"))?;
        frame.push_simple(self.id)?;
        frame.push_simple(self.reason)?;
        Ok(frame)
    }
}
