//! Contact Response
use std::net::SocketAddr;
use std::string::ToString;

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// Get the value of a key
#[derive(Debug)]
pub struct ContactResponse {
    /// Id of the InAlive peer.
    pub addrs: Vec<String>,
}

impl ContactResponse {
    /// Creates a new message
    pub fn new(addrs: Vec<SocketAddr>) -> ContactResponse {
        ContactResponse {
            addrs: addrs.into_iter().map(|addr| addr.to_string()).collect(),
        }
    }

    /// Accessor for the key
    pub fn addrs(&self) -> &[String] {
        &self.addrs
    }

    /// Extract a Set message from the parse.
    pub fn parse_frames(parse: &mut Parse) -> Result<ContactResponse, Error> {
        let count = parse.next_unsigned()? as usize;
        let mut addrs = Vec::new();
        for _ in 0..count {
            let addr = parse.next_string().unwrap();
            addrs.push(addr);
        }
        Ok(ContactResponse { addrs })
    }

    /// Convert the Contact Response into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let ContactResponse { addrs } = self;
        let mut frame = Frame::array();
        frame.push_simple(String::from("CTCT_RESP"))?;
        frame.push_unsigned(addrs.len().try_into().unwrap())?;
        addrs.into_iter().for_each(|addr| {
            frame.push_simple(addr).unwrap();
        });
        Ok(frame)
    }
}
