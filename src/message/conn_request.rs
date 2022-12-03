//! Connection Request

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// Get the value of a key
#[derive(Debug)]
pub struct ConnRequest {
    /// Id of the OutAlive peer's controller
    pub id: String,
    /// label of the OutAlive peer's controller
    pub label: String,
    /// address of the OutAlive peer's controller
    /// This can be used by the in peer to dial
    /// back in the out peer, if the connection
    /// is lost,
    pub address: String,
}

impl ConnRequest {
    /// Creates a new message
    pub fn new(id: String, label: String, address: String) -> ConnRequest {
        ConnRequest { id, label, address }
    }

    /// Accessor for the key
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Accessor for the label
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Accessor for the address
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Extract a ConnRequest message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<ConnRequest, Error> {
        let id = parse.next_string()?;
        let label = parse.next_string()?;
        let address = parse.next_string()?;
        Ok(ConnRequest { id, label, address })
    }

    /// Convert the ConnRequest into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let ConnRequest { id, label, address } = self;
        let mut frame = Frame::array();
        frame.push_simple(String::from("CONN_REQ"))?;
        frame.push_simple(id)?;
        frame.push_simple(label)?;
        frame.push_simple(address)?;
        Ok(frame)
    }
}
