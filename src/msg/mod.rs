//! Messages supported by the peer 2 peer protocol
use crate::Frame;
use crate::Parse;

pub mod error;
pub use error::Error;
pub mod conn_request;
pub use conn_request::ConnRequest;
pub mod conn_response;
pub use conn_response::ConnResponse;
pub mod conn_rejection;
pub use conn_rejection::ConnRejection;
pub mod heartbeat_request;
pub use heartbeat_request::HeartbeatRequest;
pub mod heartbeat_response;
pub use heartbeat_response::HeartbeatResponse;

/// List of P2P messages
#[derive(Debug)]
pub enum Message {
    ///  ConnRequest
    ConnRequest(ConnRequest),
    ///  ConnResponse
    ConnResponse(ConnResponse),
    /// ConnRejection
    ConnRejection(ConnRejection),
    /// Heartbeat Request
    HeartbeatRequest(HeartbeatRequest),
    /// Heartbeat Response
    HeartbeatResponse(HeartbeatResponse),
}

impl Message {
    /// Parse a message from o frame
    pub fn from_frame(frame: Frame) -> Result<Message, Error> {
        let mut parse = Parse::new(frame)?;
        let id = parse.next_string()?.to_uppercase();
        let message = match id.as_str() {
            "CONN_REQ" => Message::ConnRequest(ConnRequest::parse_frames(&mut parse)?),
            "CONN_RESP" => Message::ConnResponse(ConnResponse::parse_frames(&mut parse)?),
            "CONN_REJECT" => Message::ConnRejection(ConnRejection::parse_frames(&mut parse)?),
            "HBT_REQ" => Message::HeartbeatRequest(HeartbeatRequest::parse_frames(&mut parse)?),
            "HBT_RESP" => Message::HeartbeatResponse(HeartbeatResponse::parse_frames(&mut parse)?),
            _ => {
                return Err(Error::UnexpectedMessage { detail: id });
            }
        };
        parse.finish()?;
        Ok(message)
    }

    /// Serialize into frame.
    pub fn into_frame(self) -> Result<Frame, Error> {
        match self {
            Message::ConnRequest(request) => request.into_frame(),
            Message::ConnResponse(response) => response.into_frame(),
            Message::ConnRejection(rejection) => rejection.into_frame(),
            Message::HeartbeatRequest(request) => request.into_frame(),
            Message::HeartbeatResponse(response) => response.into_frame(),
        }
    }
}
