//! A node
use async_recursion::async_recursion;
use chrono::Utc;
use futures::sink::SinkExt;
use futures::stream::SplitSink;
use futures::stream::StreamExt;
use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::string::ToString;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::{JoinError, JoinHandle};
use tokio::time::{self, Duration};
use tokio_util::codec::Framed;
use uuid::Uuid;

use super::command::Command;
use super::event::Event;
use crate::decoder;
use crate::msg::{self, ConnRequest, ConnResponse, HeartbeatRequest, HeartbeatResponse, Message};
use crate::Frame;
use crate::FrameCodec;

/// NetworkController
#[derive(Debug)]
pub struct Peer {
    /// Unique id of the peer. It is used for internal communication
    /// between the controller and the peer.
    pub id: Uuid,
    /// id of the controller (we publish this information to remote peers)
    /// This information is used to provide identity to the peer
    pub controller: Uuid,
    /// label (it's the controller's label)
    /// This information is used to provide identity to the peer
    pub label: String,
    /// listen address of the controller
    /// This information is used to provide identity to the peer
    pub controller_addr: SocketAddr,
    /// Unique id of the peer in the network. It is an option because we
    /// don't have the id until we hear back from the peer. It is the
    /// id of the peer's network controller.
    pub peer_id: Option<Uuid>,
    /// Network Address of the peer
    pub addr: Option<SocketAddr>,
    /// Peer State
    pub state: PeerState,
    /// Sink. This is an option, because we don't have one until
    /// we establish a connection with the peer.
    pub sink: Option<SplitSink<Framed<TcpStream, FrameCodec>, Frame>>,
    // /// Stream. This is an option, because we don't have one until
    // /// we establish a connection with the peer.
    // pub stream: Option<SplitStream<Framed<TcpStream, FrameCodec>>>,
    /// This is the way to receive commands from the controller.
    pub rx_com: Receiver<Command>,
    /// This is the way to update the controller.
    pub tx_evt: Sender<Event>,
    /// This is the way to update the peer.
    pub tx_com: Sender<Command>,
    /// local addr
    pub local_addr: Option<SocketAddr>,
    /// remote addr
    pub peer_addr: Option<SocketAddr>,
    /// heartbeat
    pub heartbeat_timeout: i32,
    /// heartbeat
    pub heartbeat_period: i32,
    /// handle to a thread that will trigger a timeout
    pub heartbeat_timeout_handle: Option<JoinHandle<()>>,
    /// handle to the listen thread
    pub listen_handle: Option<JoinHandle<()>>,
    /// handle to the periodic heartbeat thread
    pub heartbeat_handle: Option<JoinHandle<()>>,
}

/// Peer Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// We know about the peer but we aren't currently doing anything with it
    Idle,
    /// We are currently trying to establish an outgoing TCP connection to the peer
    OutConnecting,
    /// We are currently handshaking with a peeer after having established and outgoing TCP connection to it
    OutHandshaking,
    /// We have an outgoing TCP connection to the peer and the handshake is done, the peer is functional
    OutAlive,
    /// We are currently handshaking with a peeer after this peer established a TCP connection towards our node
    InHandshaking,
    /// The peer has established a TCP connection towards us and the handshake is done, the peer is functional
    InAlive,
    /// We have banned this peer: we won't connect to it and will reject connection attempts from it
    Banned,
}

impl Default for PeerState {
    fn default() -> Self {
        PeerState::Idle
    }
}

impl ToString for PeerState {
    fn to_string(&self) -> String {
        match self {
            PeerState::Idle => "idle".to_owned(),
            PeerState::OutConnecting => "out connecting".to_owned(),
            PeerState::OutHandshaking => "out handshaking".to_owned(),
            PeerState::OutAlive => "out alive".to_owned(),
            PeerState::InHandshaking => "in handshaking".to_owned(),
            PeerState::InAlive => "in alive".to_owned(),
            PeerState::Banned => "banned".to_owned(),
        }
    }
}

impl Peer {
    /// Creates a new peer, in default (idle) state
    /// The created peer doesn't do anything, so you need to
    /// * either call 'connect(addr)' so that it connects to
    ///   a remote peer,
    /// * or call 'listen(stream)' so that it starts incoming
    ///   request from the connection (stream)
    pub fn new(
        controller: Uuid,
        label: String,
        controller_addr: SocketAddr,
        tx_evt: Sender<Event>,
        tx_com: Sender<Command>,
        rx_com: Receiver<Command>,
        heartbeat_timeout: i32,
        heartbeat_period: i32,
    ) -> Peer {
        Peer {
            id: Uuid::new_v4(),
            controller,
            label,
            controller_addr,
            peer_id: None,
            addr: None,
            state: PeerState::Idle,
            sink: None,
            local_addr: None,
            peer_addr: None,
            tx_evt,
            tx_com,
            rx_com,
            heartbeat_timeout,
            heartbeat_period,
            heartbeat_timeout_handle: None,
            listen_handle: None,
            heartbeat_handle: None,
        }
    }

    /// We want the peer to establish a TCP connection.
    /// If everything goes well, the peer exits in the OutConnecting state
    async fn connect(&mut self, addr: &SocketAddr, attempt: i32) -> Result<(), Error> {
        // Guarding against invalid state.
        // The peer can be either in
        // * idle state (it hasn't tried to connect yet.)
        // * out-connecting (it has already tried to connect, but wasn't successful.)
        if (self.state != PeerState::Idle) && (self.state != PeerState::OutConnecting) {
            let msg = Event::InvalidState {
                id: self.id,
                expected: vec![PeerState::Idle, PeerState::OutConnecting],
                actual: self.state,
            };
            if let Err(err) = self.tx_evt.send(msg).await {
                // We're in deep trouble here, we can't communicate with
                // the network controller. So we shutdown.
                return Err(Error::SendEvent {
                    source: err,
                    detail: "Peer could not send invalid state to controller. receiver dropped"
                        .to_owned(),
                });
            } else {
                return Ok(());
            }
        }
        self.state = PeerState::OutConnecting;
        log::trace!(
            "Peer {} | is OutConnecting",
            self.id.to_string().get(0..8).unwrap()
        );
        log::info!(
            "Peer {} | trying to connect to {} (attempt {})",
            self.id.to_string().get(0..8).unwrap(),
            addr,
            attempt,
        );
        let stream = match TcpStream::connect(addr).await {
            // FIXME Unwrap
            Ok(stream) => stream,
            Err(err) => {
                let msg = Event::ConnectionError {
                    id: self.id,
                    addr: *addr,
                    source: err,
                };
                if let Err(err) = self.tx_evt.send(msg).await {
                    return Err(Error::SendEvent {
                        source: err,
                        detail:
                            "Peer could not send connection error to controller. receiver dropped"
                                .to_owned(),
                    });
                } else {
                    return Ok(());
                }
            }
        };

        self.addr = Some(*addr);
        self.local_addr = Some(stream.local_addr().expect("local addr"));
        self.peer_addr = Some(stream.peer_addr().expect("peer addr"));

        let frames = Framed::new(stream, FrameCodec);

        let (sink, mut stream) = frames.split();

        self.sink = Some(sink);

        let id = self.id;
        let tx_com = self.tx_com.clone();

        let handle = tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                match frame {
                    Ok(frame) => {
                        let msg = Message::from_frame(frame).expect("Message decoding from frame");
                        if let Err(err) = handle_message(id, msg, tx_com.clone()).await {
                            log::error!("Error handling a message: {err}");
                        }
                    }
                    Err(err) => {
                        log::error!("Error from message stream {}", err);
                    }
                }
            }
        });

        self.listen_handle = Some(handle);

        log::trace!(
            "Connection {} <=> {}",
            self.local_addr.unwrap(),
            self.peer_addr.unwrap()
        );
        let msg = Event::Connected { id: self.id };
        if let Err(err) = self.tx_evt.send(msg).await {
            return Err(Error::SendEvent {
                source: err,
                detail: "Peer could not send 'connected' to controller. receiver dropped"
                    .to_owned(),
            });
        }
        Ok(())
    }

    /// We want the peer to listen on the given TcpStream
    /// If everything goes well, the peer exits in the InHandshaking state
    async fn listen(&mut self, stream: TcpStream) -> Result<(), Error> {
        // Guarding against invalid state.
        // The peer can be either in
        // * idle state (it hasn't tried to connect yet.)
        // * out-connecting (it has already tried to connect, but wasn't successful.)
        if self.state != PeerState::Idle {
            let msg = Event::InvalidState {
                id: self.id,
                expected: vec![PeerState::Idle],
                actual: self.state,
            };
            if let Err(err) = self.tx_evt.send(msg).await {
                // We're in deep trouble here, we can't communicate with
                // the network controller. So we shutdown.
                return Err(Error::SendEvent {
                    source: err,
                    detail: "Peer could not send 'invalid state' to controller. receiver dropped"
                        .to_owned(),
                });
            } else {
                return Ok(());
            }
        }

        self.state = PeerState::InHandshaking;
        log::trace!(
            "Peer {} | is InHandshaking",
            self.id.to_string().get(0..8).unwrap()
        );
        self.local_addr = Some(stream.local_addr().expect("local addr"));
        self.peer_addr = Some(stream.peer_addr().expect("peer addr"));

        log::info!(
            "Peer {} | listening on {}",
            self.id.to_string().get(0..8).unwrap(),
            self.peer_addr.unwrap(),
        );

        let frames = Framed::new(stream, FrameCodec);

        let (sink, mut stream) = frames.split();

        self.sink = Some(sink);

        let tx_com = self.tx_com.clone();

        let id = self.id;
        let handle = tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                match frame {
                    Ok(frame) => {
                        let msg = Message::from_frame(frame).expect("Message decoding from frame");
                        if let Err(err) = handle_message(id, msg, tx_com.clone()).await {
                            log::error!("Error handling a message: {err}");
                        }
                    }
                    Err(err) => {
                        log::error!("Error from message stream {}", err);
                    }
                }
            }
        });

        self.listen_handle = Some(handle);
        log::info!(
            "Connection {} <=> {}",
            self.local_addr.unwrap(),
            self.peer_addr.unwrap()
        );
        let msg = Event::Listening { id: self.id };
        if let Err(err) = self.tx_evt.send(msg).await {
            return Err(Error::SendEvent {
                source: err,
                detail: format!(
                    "Peer {} | Could not send 'listening' to controller | receiver dropped.",
                    self.id.to_string().get(0..8).unwrap()
                ),
            });
        }
        Ok(())
    }

    /// The main peer loop:
    /// We listen to commands from the network controller, and perform the
    pub async fn run(mut self) -> Result<(), Error> {
        log::trace!("Peer {} | running", self.id.to_string().get(0..8).unwrap());
        while let Some(cmd) = self.rx_com.recv().await {
            if let Err(err) = self.handle_command(cmd).await {
                log::warn!(
                    "Peer {} | Could not process command in main loop | {err} | => Terminating",
                    self.id.to_string().get(0..8).unwrap(),
                );
                match self.state {
                    PeerState::InAlive | PeerState::InHandshaking => self.terminate().await?,
                    PeerState::OutAlive | PeerState::OutHandshaking | PeerState::OutConnecting => {
                        self.disconnect().await?
                    }
                    _ => {
                        log::warn!(
                            "Peer {} | Could not process command in main loop | {err} | => Terminating",
                            self.id.to_string().get(0..8).unwrap(),
                            );
                    }
                }
            }
        }
        Ok(())
    }

    async fn abort_threads(&mut self) -> Result<(), Error> {
        log::info!(
            "Peer {} | Aborting threads.",
            self.id.to_string().get(0..8).unwrap()
        );
        if let Some(handle) = &self.listen_handle {
            handle.abort();
            self.listen_handle = None;
        }
        if let Some(handle) = &self.heartbeat_timeout_handle {
            handle.abort();
            self.heartbeat_timeout_handle = None;
        }
        if let Some(handle) = &self.heartbeat_handle {
            handle.abort();
            self.heartbeat_handle = None;
        }
        Ok(())
    }

    async fn close_receiver(&mut self) -> Result<(), Error> {
        log::info!(
            "Peer {} | Closing receiver.",
            self.id.to_string().get(0..8).unwrap()
        );

        // Now we're closing the receiver for the main loop. Per Tokio's documentation,
        // we still need to process the messages in the pipe. So for each outstanding
        // command, we call 'handle_command', which may call 'terminate' => async-recursion
        self.rx_com.close();
        while let Some(cmd) = self.rx_com.recv().await {
            if let Err(err) = self.handle_command(cmd).await {
                log::warn!(
                    "Peer {} | Could not process command while terminating | {err}",
                    self.id.to_string().get(0..8).unwrap()
                );
            }
        }
        Ok(())
    }

    async fn terminate(&mut self) -> Result<(), Error> {
        log::info!(
            "Peer {} | Terminating",
            self.id.to_string().get(0..8).unwrap()
        );
        self.abort_threads().await?;

        self.close_receiver().await?;

        log::info!(
            "Peer {} | Main loop is closed",
            self.id.to_string().get(0..8).unwrap()
        );
        let msg = Event::Terminated { id: self.id };
        if let Err(err) = self.tx_evt.send(msg).await {
            return Err(Error::SendEvent {
                source: err,
                detail: format!(
                    "Peer {} | Could not send 'terminated' to controller | Receiver dropped",
                    self.id.to_string().get(0..8).unwrap()
                ),
            });
        }
        self.state = PeerState::Idle;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        log::info!(
            "Peer {} | Disconnecting",
            self.id.to_string().get(0..8).unwrap()
        );
        self.abort_threads().await?;

        self.close_receiver().await?;

        log::info!(
            "Peer {} | Main loop is closed",
            self.id.to_string().get(0..8).unwrap()
        );
        if self.addr.is_none() {
            return Err(Error::InvalidAddr {
                detail: format!(
                    "Peer {} | Should have an address.",
                    self.id.to_string().get(0..8).unwrap()
                ),
            });
        }
        let msg = Event::Disconnected {
            id: self.id,
            addr: self.addr.unwrap(), // safe we tested above.
        };
        if let Err(err) = self.tx_evt.send(msg).await {
            return Err(Error::SendEvent {
                source: err,
                detail: format!(
                    "Peer {} | Could not send 'disconnectted' to controller | Receiver dropped",
                    self.id.to_string().get(0..8).unwrap()
                ),
            });
        }
        self.state = PeerState::Idle;
        Ok(())
    }

    #[async_recursion]
    async fn handle_command(&mut self, command: Command) -> Result<(), Error> {
        match (self.state, command) {
            (PeerState::Idle, Command::Connect { addr, attempt }) => {
                // We receive an address to connect to from the controller.
                self.connect(&addr, attempt).await
            }
            (PeerState::Idle, Command::Listen { stream }) => {
                // The controller has a socket on which the peer
                // need to listen.
                self.listen(stream).await
            }
            (PeerState::OutConnecting, Command::Connect { addr, attempt }) => {
                // We're just retrying to connect
                self.connect(&addr, attempt).await
            }
            (PeerState::OutConnecting, Command::SendConnRequest) => {
                // The controller has asked to start handshaking with the remote peer.
                // So we change our state to out-handshaking, and send a conn-request.
                // TODO We need to check if we don't have too many connections,
                self.state = PeerState::OutHandshaking;
                let frame = Message::ConnRequest(ConnRequest::new(
                    self.controller,
                    self.label.clone(),
                    self.controller_addr,
                ))
                .into_frame()
                .map_err(|err| Error::Message { source: err })?;
                self.sink
                    .as_mut()
                    .unwrap()
                    .send(frame)
                    .await
                    .map_err(|err| Error::Codec { source: err })
            }
            (
                PeerState::InHandshaking,
                Command::SendConnResponse {
                    peer_id,
                    peer_label,
                    peer_addr,
                },
            ) => {
                // Our listening thread has received a connection request,
                // so we send back a connection response. Then we cross fingers,
                // because we're expecting the message to arrive, so we
                // set the state to InAlive, and notify the controller.
                let frame =
                    Message::ConnResponse(ConnResponse::new(self.controller, self.label.clone()))
                        .into_frame()
                        .map_err(|err| Error::Message { source: err })?;
                self.sink
                    .as_mut()
                    .unwrap()
                    .send(frame)
                    .await
                    .map_err(|err| Error::Codec { source: err })?;
                self.state = PeerState::InAlive;
                let event = Event::InAlive {
                    id: self.id,
                    peer_id: Uuid::parse_str(&peer_id).unwrap(),
                    peer_label,
                    peer_addr: SocketAddr::from_str(&peer_addr).unwrap(),
                };
                if let Err(err) = self.tx_evt.send(event).await {
                    // We're in deep trouble here, we can't communicate with
                    // the network controller. So we shutdown.
                    return Err(Error::SendEvent {
                        source: err,
                        detail: format!(
                            "Peer {} | Could not send 'established' to controller | Receiver dropped",
                            self.id.to_string().get(0..8).unwrap()
                        ),
                    });
                }
                Ok(())
            }
            (
                PeerState::OutHandshaking,
                Command::FinalizeConn {
                    peer_id,
                    peer_label,
                },
            ) => {
                // We're done with the connection setup, now we're Alive.
                // Change our state
                // Notify the controller (not sure if its necessary, but its good tell the boss you're alive)
                // TODO Start a thread to send regular heartbeat to remote peer to check connection
                // health.
                self.state = PeerState::OutAlive;
                let event = Event::OutAlive {
                    id: self.id,
                    peer_id: Uuid::parse_str(&peer_id).unwrap(),
                    peer_label,
                    peer_addr: self.addr.unwrap(),
                };
                if let Err(err) = self.tx_evt.send(event).await {
                    // We're in deep trouble here, we can't communicate with
                    // the network controller. So we shutdown.
                    return Err(Error::SendEvent {
                        source: err,
                        detail: format!(
                            "Peer {} | Could not send 'out alive' to controller | Receiver dropped",
                            self.id.to_string().get(0..8).unwrap()
                        ),
                    });
                }
                let handle = self.heartbeats().await?;
                self.heartbeat_handle = Some(handle);
                Ok(())
            }
            (PeerState::OutAlive, Command::HeartbeatRequest) => {
                // We have received a periodic tick, and need to send a heartbeat request
                // We also store a handle to a detached thread that will trigger a timeout
                // if we haven't received a response before a configurable duration.
                let frame =
                    Message::HeartbeatRequest(HeartbeatRequest::now(self.id, self.label.clone()))
                        .into_frame()
                        .map_err(|err| Error::Message { source: err })?;
                self.sink
                    .as_mut()
                    .unwrap()
                    .send(frame)
                    .await
                    .map_err(|err| {
                        log::warn!(
                            "Peer {} | Could not send 'heartbeat request' to remote | {err}",
                            self.id.to_string().get(0..8).unwrap()
                        );
                        Error::Codec { source: err }
                    })?;
                log::trace!(
                    "Peer {} | Sent a 'heartbeat request'",
                    self.id.to_string().get(0..8).unwrap()
                );
                let timeout = self.heartbeat_timeout.try_into().unwrap();
                let tx = self.tx_com.clone();
                let id = self.id;
                let handle = tokio::spawn(async move {
                    time::sleep(Duration::from_secs(timeout)).await;
                    if let Err(err) = tx.send(Command::HeartbeatTimeout).await {
                        log::error!(
                            "Peer {} | Could not send 'heartbeat timout' to itself | Receiver dropped | {err}",
                            id.to_string().get(0..8).unwrap()
                            );
                    }
                });
                log::trace!(
                    "Peer {} | Storing a heartbeat handle",
                    self.id.to_string().get(0..8).unwrap()
                );
                if let Some(old_handle) = &self.heartbeat_timeout_handle {
                    old_handle.abort(); // TODO Not sure what the correct behavior should be.
                }
                self.heartbeat_timeout_handle = Some(handle);
                Ok(())
            }
            (PeerState::OutAlive, Command::CancelHeartbeatTimeout { src: _ }) => {
                // We have received a request to remove a task that would
                // trigger a timeout
                if let Some(handle) = &self.heartbeat_timeout_handle {
                    handle.abort();
                    self.heartbeat_timeout_handle = None;
                } else {
                    log::warn!(
                        "Peer {} | Could not find handle for heartbeat timeout.",
                        self.id.to_string().get(0..8).unwrap()
                    );
                }
                Ok(())
            }
            (PeerState::OutAlive, Command::HeartbeatTimeout) => {
                // We have received a heartbeat timeout. Remote is not reachable => disconnect
                // Note that it is unlikely we end up here: The timeout delay is > heartbeat
                // period. So this peer will most likely disconnect because it cannot send
                // heartbeat request to the remote, rather than a heartbeat timeout
                log::warn!(
                    "Peer {} | Heartbeat timeout | Disconnecting",
                    self.id.to_string().get(0..8).unwrap()
                );
                self.disconnect().await
            }
            (PeerState::InAlive, Command::HeartbeatTimeout) => {
                // We have received a heartbeat timeout. Remote is not reachable => terminate
                log::warn!(
                    "Peer {} | Heartbeat timeout | Terminating",
                    self.id.to_string().get(0..8).unwrap()
                );
                self.terminate().await
            }
            (PeerState::InAlive, Command::HeartbeatResponse { src }) => {
                // We have received a heartbeat request, and are
                // asked to send a response back.
                // We also store a handle to a thread
                let frame = Message::HeartbeatResponse(HeartbeatResponse::now(
                    self.controller,
                    self.label.clone(),
                    src,
                ))
                .into_frame()
                .map_err(|err| Error::Message { source: err })?;
                self.sink
                    .as_mut()
                    .unwrap()
                    .send(frame)
                    .await
                    .map_err(|err| Error::Codec { source: err })?;
                log::trace!(
                    "Peer {} | Sent a heartbeat response.",
                    self.id.to_string().get(0..8).unwrap()
                );
                let timeout = self.heartbeat_timeout.try_into().unwrap();
                let tx = self.tx_com.clone();
                let id = self.id;
                let handle = tokio::spawn(async move {
                    time::sleep(Duration::from_secs(timeout)).await;
                    if let Err(err) = tx.send(Command::HeartbeatTimeout).await {
                        log::error!(
                            "Peer {} | Could not send 'heartbeat timout' to itself. receiver dropped: {err}",
                            id.to_string().get(0..8).unwrap()
                            );
                    }
                });
                if let Some(old_handle) = &self.heartbeat_timeout_handle {
                    old_handle.abort();
                }
                self.heartbeat_timeout_handle = Some(handle);
                Ok(())
            }
            (state, command) => {
                log::info!(
                    "Peer {} | Unhandled command '{}' in state '{}'",
                    self.id.to_string().get(0..8).unwrap(),
                    command.to_string(),
                    state.to_string(),
                );
                Ok(())
            }
        }
    }

    async fn heartbeats(&self) -> Result<JoinHandle<()>, Error> {
        let tx = self.tx_com.clone();
        let period = self.heartbeat_period.try_into().unwrap();
        let id = self.id;
        let handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(period));
            loop {
                interval.tick().await;
                if let Err(err) = tx.send(Command::HeartbeatRequest).await {
                    log::error!(
                        "Peer {} | Could not send 'heartbeat request' to itself | Receiver dropped | {err}",
                        id.to_string().get(0..8).unwrap(),
                        );
                    return;
                }
            }
        });
        Ok(handle)
    }
}

async fn handle_message(id: Uuid, msg: Message, tx: Sender<Command>) -> Result<(), Error> {
    match msg {
        Message::ConnRequest(conn_request) => {
            // The InPeer receives this message
            log::info!(
                "Peer {} | Received a 'connection request' from {}",
                id.to_string().get(0..8).unwrap(),
                conn_request.id.get(0..8).unwrap()
            );
            if let Err(err) = tx
                .send(Command::SendConnResponse {
                    peer_id: conn_request.id().to_owned(),
                    peer_label: conn_request.label().to_owned(),
                    peer_addr: conn_request.address().to_owned(),
                })
                .await
            {
                log::error!(
                        "Peer {} | Could not send 'connection response' to itself | Receiver dropped | {err}",
                        id.to_string().get(0..8).unwrap()
                        );
            }
        }
        Message::ConnResponse(conn_response) => {
            log::info!(
                "Peer {} | Received a connection response from {}",
                id.to_string().get(0..8).unwrap(),
                conn_response.id.get(0..8).unwrap()
            );
            if let Err(err) = tx
                .send(Command::FinalizeConn {
                    peer_id: conn_response.id().to_owned(),
                    peer_label: conn_response.label().to_owned(),
                })
                .await
            {
                log::error!(
                        "Peer {} | Could not send 'connection finalization' to itself | Receiver dropped | {err}",
                        id.to_string().get(0..8).unwrap()
                        );
            }
        }
        Message::ConnRejection(_conn_rejection) => {
            log::info!(
                "Peer {} | Received a 'connection rejection'",
                id.to_string().get(0..8).unwrap()
            );
        }
        Message::HeartbeatRequest(heartbeat_request) => {
            log::trace!(
                "Peer {} | Received a 'heartbeat request'",
                id.to_string().get(0..8).unwrap()
            );
            // We send a command to respond, and we embed the original
            // timestamp so that it can be forwarded back to the origin.
            tx.send(Command::HeartbeatResponse {
                src: heartbeat_request.src(),
            })
            .await
            .expect("Cannot send command to self");
        }
        Message::HeartbeatResponse(heartbeat_response) => {
            let dt = Utc::now();
            let ts = dt.timestamp_micros();
            let rtt = ts - heartbeat_response.src();
            log::info!(
                "Peer {} | Received a 'heartbeat response' from {} | RTT {} μs",
                id.to_string().get(0..8).unwrap(),
                heartbeat_response.label(),
                rtt
            );
            tx.send(Command::CancelHeartbeatTimeout {
                src: heartbeat_response.src(),
            })
            .await
            .expect("Cannot send command to self");
        }
    }
    Ok(())
}

/// Error type for the peer
#[derive(Debug)]
pub enum Error {
    /// The peer could not send an event to the controller.
    SendEvent {
        /// Source
        source: mpsc::error::SendError<Event>,
        /// Error detail
        detail: String,
    },

    /// The peer could not send a command to itself
    SendCommand {
        /// Source
        source: mpsc::error::SendError<Command>,
        /// Error detail
        detail: String,
    },

    /// Codec Error
    Codec {
        /// Just the source
        source: decoder::Error,
    },

    /// Message Error
    Message {
        /// Just the source
        source: msg::Error,
    },

    /// Thread Error
    Thread {
        /// Just the source
        source: JoinError,
    },

    /// InvalidAddr
    InvalidAddr {
        /// detail
        detail: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SendEvent { source: _, detail } => {
                write!(f, "Peer could not send event to controller {}", detail)
            }
            Error::SendCommand { source: _, detail } => {
                write!(f, "Peer could not send command to itself {}", detail)
            }
            Error::Codec { source: _ } => {
                write!(f, "Codec Error")
            }
            Error::Message { source: _ } => {
                write!(f, "Could not create a message to send to remote peer.")
            }
            Error::Thread { source: _ } => {
                write!(f, "Thread Error")
            }
            Error::InvalidAddr { detail: _ } => {
                write!(f, "Invalid Addr")
            }
        }
    }
}
