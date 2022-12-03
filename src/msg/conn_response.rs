//! Connection Response

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// Get the value of a key
#[derive(Debug)]
pub struct ConnResponse {
    /// Id of the InAlive peer.
    pub id: String,
    /// label of the InAlive peer.
    pub label: String,
}

impl ConnResponse {
    /// Creates a new message
    pub fn new(id: impl ToString, label: String) -> ConnResponse {
        ConnResponse {
            id: id.to_string(),
            label,
        }
    }

    /// Accessor for the key
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Accessor for the label
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Extract a Set message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<ConnResponse, Error> {
        let id = parse.next_string()?;
        let label = parse.next_string()?;
        Ok(ConnResponse { id, label })
    }

    /// Convert the Connection Response into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let ConnResponse { id, label } = self;
        let mut frame = Frame::array();
        frame.push_simple(String::from("CONN_RESP"))?;
        frame.push_simple(id)?;
        frame.push_simple(label)?;
        Ok(frame)
    }
}
