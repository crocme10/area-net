//! Command are sent to the peer.
use std::net::SocketAddr;
use tokio::net::TcpStream;

/// Commands issued by the network controller to the peers
#[derive(Debug)]
pub enum Command {
    /// Peer has to initiate a TcpStream connection
    /// TODO Do we need to add a timeout?
    Connect {
        /// address to connect to
        addr: SocketAddr,
        /// attempt
        attempt: i32,
    },
    /// Listen to messages from remote peer
    Listen {
        /// TcpStream
        stream: TcpStream,
    },
    /// Send a Connection Request (for Handshake)
    SendConnRequest,
    /// Send a Connection Response (for Handshake)
    SendConnResponse {
        /// peer id
        peer_id: String,
        /// peer label
        peer_label: String,
        /// peer addr
        peer_addr: String,
    },
    /// Finalize the connection
    FinalizeConn {
        /// peer id
        peer_id: String,
        /// peer label
        peer_label: String,
    },
    /// Send a heartbeat request
    HeartbeatRequest,
    /// Send a heartbeat response
    HeartbeatResponse {
        /// To send the heartbeat response,
        /// we need to provide the timestamp
        /// that was given in the heartbeat
        /// request.
        src: i64,
    },
    /// We missed a heartbeat
    HeartbeatTimeout,
    /// Peer has received a HeartbeatResponse
    /// We need to remove the heartbeat timeout thread
    /// We need to store the rtt
    CancelHeartbeatTimeout {
        /// Round Trip Time in microseconds.
        rtt: i64,
    },
    /// Request the peer to send a ContactRequest to its remote.
    SendContactRequest,
    /// Request the peer to ask the controller for contacts.
    RequestContacts,
    /// Request the peer to send a ContactResponse to its remote
    SendContactResponse {
        /// list of addresses to send to the remote
        addrs: Vec<SocketAddr>,
    },
    /// Request the peer to senda ContactUpdated to the controller
    UpdateContacts {
        /// list of addresses to send to the controller
        addrs: Vec<SocketAddr>,
    },
    /// At anypoint we can ask the peer to terminate the connection with the remote peer.
    Disconnect,
    /// Ask the peer to terminate itself.
    Terminate,
}

impl ToString for Command {
    fn to_string(&self) -> String {
        match self {
            Command::Connect {
                addr: _,
                attempt: _,
            } => "connect".to_owned(),
            Command::Listen { stream: _ } => "listen".to_owned(),
            Command::SendConnRequest {} => "connection request".to_owned(),
            Command::SendConnResponse {
                peer_id: _,
                peer_label: _,
                peer_addr: _,
            } => "connection response".to_owned(),
            Command::FinalizeConn {
                peer_id: _,
                peer_label: _,
            } => "connection finalization".to_owned(),
            Command::HeartbeatResponse { src: _ } => "heartbeat response".to_owned(),
            Command::HeartbeatRequest => "heartbeat request".to_owned(),
            Command::HeartbeatTimeout => "heartbeat timeout".to_owned(),
            Command::CancelHeartbeatTimeout { rtt: _ } => "cancel heartbeat timeout".to_owned(),
            Command::SendContactRequest => "contact request".to_owned(),
            Command::SendContactResponse { addrs: _ } => "contact response".to_owned(),
            Command::RequestContacts => "request contacts".to_owned(),
            Command::UpdateContacts { addrs: _ } => "update contacts".to_owned(),
            Command::Disconnect => "disconnect".to_owned(),
            Command::Terminate => "terminate".to_owned(),
        }
    }
}
