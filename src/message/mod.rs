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
pub mod contact_request;
pub use contact_request::ContactRequest;
pub mod contact_response;
pub use contact_response::ContactResponse;

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
    /// Contact Request
    ContactRequest(ContactRequest),
    /// Contact Response
    ContactResponse(ContactResponse),
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
            "CTCT_REQ" => Message::ContactRequest(ContactRequest::parse_frames(&mut parse)?),
            "CTCT_RESP" => Message::ContactResponse(ContactResponse::parse_frames(&mut parse)?),
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
            Message::ContactRequest(request) => request.into_frame(),
            Message::ContactResponse(response) => response.into_frame(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[test]
    fn should_encode_decode_connection_request() {
        let msg_in = Message::ConnRequest(ConnRequest::new(
            "id".into(),
            "bob".into(),
            "[::1]:8000".into(),
        ));
        let frame = msg_in.into_frame().unwrap();
        if let Message::ConnRequest(response) = Message::from_frame(frame).unwrap() {
            assert_eq!(response.id, "id");
            assert_eq!(response.label, "bob");
            assert_eq!(response.address, "[::1]:8000");
        } else {
            panic!("Message from frame should be a ConnRequest");
        }
    }

    #[test]
    fn should_encode_decode_connection_response() {
        let msg_in = Message::ConnResponse(ConnResponse::new("id".into(), "bob".into()));
        let frame = msg_in.into_frame().unwrap();
        if let Message::ConnResponse(response) = Message::from_frame(frame).unwrap() {
            assert_eq!(response.id, "id");
            assert_eq!(response.label, "bob");
        } else {
            panic!("Message from frame should be a ConnResponse");
        }
    }

    #[test]
    fn should_encode_decode_heartbeat_request() {
        let msg_in = Message::HeartbeatRequest(HeartbeatRequest::now("id".into(), "bob".into()));
        let frame = msg_in.into_frame().unwrap();
        if let Message::HeartbeatRequest(response) = Message::from_frame(frame).unwrap() {
            assert_eq!(response.id, "id");
            assert_eq!(response.label, "bob");
        } else {
            panic!("Message from frame should be a HeartbeatRequest");
        }
    }

    #[test]
    fn should_encode_decode_heartbeat_response() {
        let msg_in =
            Message::HeartbeatResponse(HeartbeatResponse::now("id".into(), "bob".into(), 42));
        let frame = msg_in.into_frame().unwrap();
        if let Message::HeartbeatResponse(response) = Message::from_frame(frame).unwrap() {
            assert_eq!(response.id, "id");
            assert_eq!(response.label, "bob");
        } else {
            panic!("Message from frame should be a HeartbeatRequest");
        }
    }

    #[test]
    fn should_encode_decode_contact_request() {
        let msg_in = Message::ContactRequest(ContactRequest);
        let frame = msg_in.into_frame().unwrap();
        match Message::from_frame(frame).unwrap() {
            Message::ContactRequest(_) => {}
            _ => {
                panic!("Message from frame should be a ContactRequest");
            }
        }
    }

    #[test]
    fn should_encode_decode_contact_response() {
        let addrs = vec!["[::1]:8090", "[::1]:8085"];
        let sock_addrs = addrs
            .iter()
            .map(|addr| SocketAddr::from_str(addr).unwrap())
            .collect::<Vec<_>>();
        let msg_in = Message::ContactResponse(ContactResponse::new(sock_addrs));
        let frame = msg_in.into_frame().unwrap();
        println!("frame: {frame:?}");
        if let Message::ContactResponse(response) = Message::from_frame(frame).unwrap() {
            assert_eq!(response.addrs, addrs);
        } else {
            panic!("Message from frame should be a ContactResponse");
        }
    }
}
