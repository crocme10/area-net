//! Contactection Request

use super::error::Error;
use crate::Frame;
use crate::Parse;

/// Get the value of a key
#[derive(Debug)]
pub struct ContactRequest;

impl ContactRequest {
    /// Extract a ContactRequest message from the parse.
    pub fn parse_frames(_parse: &mut Parse) -> Result<ContactRequest, Error> {
        Ok(ContactRequest)
    }

    /// Convert the ContactRequest into a frame
    pub fn into_frame(self) -> Result<Frame, Error> {
        let ContactRequest = self;
        let mut frame = Frame::array();
        frame.push_string(String::from("CTCT_REQ"))?;
        Ok(frame)
    }
}
