//! A network controller

use std::net::SocketAddr;
use uuid::Uuid;

use super::peer::PeerState;

/// Event are messages sent to the network controller.
#[derive(Debug)]
pub enum Event {
    /// Bind Error is sent by the network controller's serve thread to
    /// indicate it could not bind the address.
    BindError {
        /// source
        source: std::io::Error,
        /// addr
        addr: SocketAddr,
    },

    /// We sent a command to a peer, but the peer is not in a state where
    /// he can accept that command.
    InvalidState {
        /// id of the peer
        id: Uuid,
        /// expected state. The peer should be in one of those state.
        expected: Vec<PeerState>,
        /// actual state
        actual: PeerState,
    },

    /// The peer is in OutConnecting state, and has successfully established
    /// a TcpStream connection. It is about to start Handshaking
    Connected {
        /// id of the peer
        id: Uuid,
    },

    /// The peer is in InHandshaking state, and has successfully established
    /// a TcpStream connection
    Listening {
        /// id of the peer
        id: Uuid,
    },

    /// The peer has completed its handshake
    OutAlive {
        /// id of the peer
        id: Uuid,
        /// remote id
        peer_id: Uuid,
        /// remote label
        peer_label: String,
        /// remote address
        peer_addr: SocketAddr,
    },

    /// The peer has completed its handshake
    InAlive {
        /// id of the peer
        id: Uuid,
        /// remote id
        peer_id: Uuid,
        /// remote label
        peer_label: String,
        /// remote address
        peer_addr: SocketAddr,
    },

    /// The peer cannot establish a TcpStream connection
    ConnectionError {
        /// id of the peer
        id: Uuid,
        /// address we tried to connect to
        addr: SocketAddr,
        /// error
        source: std::io::Error,
    },

    /// The peer updates the controller about the health
    /// of the connection
    /// Currently only the out peer sends this update
    /// so that the controller can in turn store the
    /// new RTT
    ConnectionUpdate {
        /// id of the peer
        id: Uuid,
        /// address we tried to connect to
        rtt: i64,
    },

    /// The peer has successfully terminated.
    Terminated {
        /// id of the peer
        id: Uuid,
    },

    /// The (out) peer has successfully closed its Tcp connection.
    Disconnected {
        /// id of the peer
        id: Uuid,
        /// address the peer was connected to.
        addr: SocketAddr,
    },
}
