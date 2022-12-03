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
        /// Origin timestamp
        src: i64,
    },
    /// We missed a heartbeat
    HeartbeatTimeout,
    /// Remove Heartbeat with given src key.
    CancelHeartbeatTimeout {
        /// Origin timestamp
        src: i64,
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
            Command::CancelHeartbeatTimeout { src: _ } => "cancel heartbeat timeout".to_owned(),
            Command::Disconnect => "disconnect".to_owned(),
            Command::Terminate => "terminate".to_owned(),
        }
    }
}
