//! Heartbeat Request
use chrono::Utc;

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// This message is sent by an InAlive peer to an OutAlive peer in
/// response to a HeartbeatRequest message.
/// we copy the field 'src' from the request to the response.
#[derive(Debug)]
pub struct HeartbeatResponse {
    /// id of the InAlive peer.
    pub id: String,
    /// label of the InAlive peer.
    pub label: String,
    /// timestamp (micros) when the message was sent from the OutAlive peer
    pub src: i64,
    /// timestamp (micros) when the message was sent from the InAlive peer
    pub dst: i64,
}

impl HeartbeatResponse {
    /// Creates a new message
    pub fn now(id: String, label: String, src: i64) -> HeartbeatResponse {
        let dt = Utc::now();
        HeartbeatResponse {
            id,
            label,
            src,
            dst: dt.timestamp_micros(),
        }
    }

    /// Accessor for the id
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Accessor for the label
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Accessor for the source timestamp
    pub fn src(&self) -> i64 {
        self.src
    }

    /// Accessor for the destination timestamp
    pub fn dst(&self) -> i64 {
        self.dst
    }

    /// Extract a Heartbeat Response message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<HeartbeatResponse, Error> {
        let id = parse.next_string()?;
        let label = parse.next_string()?;
        let src = parse.next_integer()?;
        let dst = parse.next_integer()?;
        Ok(HeartbeatResponse {
            id,
            label,
            src,
            dst,
        })
    }

    /// Convert the Heartbeat Request into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let HeartbeatResponse {
            id,
            label,
            src,
            dst,
        } = self;
        let mut frame = Frame::array();
        frame.push_string(String::from("HBT_RESP"))?;
        frame.push_string(id)?;
        frame.push_string(label)?;
        frame.push_integer(src)?;
        frame.push_integer(dst)?;
        Ok(frame)
    }
}
