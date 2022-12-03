//! Heartbeat Request
use chrono::Utc;

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// This message is sent by an OutAlive peer to an InAlive peer to
/// check the connection health. The OutAlive peer expect a
/// HeartbeatResponse in return.
#[derive(Debug)]
pub struct HeartbeatRequest {
    /// id of the OutAlive peer.
    pub id: String,
    /// label of the OutAlive peer.
    pub label: String,
    /// timestamp (micros) when the message was sent by the OutAlive peer.
    /// This is used to estimate Round Trip Time (RTT)
    pub src: i64,
}

impl HeartbeatRequest {
    /// Creates a new message
    pub fn now(id: String, label: String) -> HeartbeatRequest {
        let dt = Utc::now();
        HeartbeatRequest {
            id,
            label,
            src: dt.timestamp_micros(),
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

    /// Accessor for the key
    pub fn src(&self) -> i64 {
        self.src
    }

    /// Extract a Heartbeat Request message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<HeartbeatRequest, Error> {
        let id = parse.next_string()?;
        let label = parse.next_string()?;
        let src = parse.next_integer()?;
        Ok(HeartbeatRequest { id, label, src })
    }

    /// Convert the Heartbeat Request into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let HeartbeatRequest { id, label, src } = self;
        let mut frame = Frame::array();
        frame.push_simple(String::from("HBT_REQ"))?;
        frame.push_simple(id)?;
        frame.push_simple(label)?;
        frame.push_integer(src)?;
        Ok(frame)
    }
}
