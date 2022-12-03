//! A network controller
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::net::{AddrParseError, IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};
use uuid::Uuid;

use super::command::Command;
use super::event::Event;
use super::peer::{self, Peer};

/// Data used to track idle information:
#[derive(Debug, Clone)]
pub struct AddrInfo {
    /// Current list of addrs we need to connect to.
    pub addr: SocketAddr,
    /// Number of time this address has been attempted.
    /// Maybe use AtomicI32 because its a counter
    pub attempt: Arc<Mutex<i32>>,
}

// We need to implement this trait because
// we use a HashSet<AddrInfo>
impl PartialEq for AddrInfo {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl Eq for AddrInfo {}

impl Hash for AddrInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

/// Network Controller State for idle
#[derive(Debug, Default)]
pub struct IdleState {
    /// Current list of addrs we need to connect to.
    pub addrs: HashSet<AddrInfo>,
}

/// Data used to track idle information:
#[derive(Debug, Clone, Serialize)]
pub struct ConnInfo {
    /// Address of the remote peer.
    pub addr: SocketAddr,
    /// Id of the remote peer
    pub id: Uuid,
    /// Label of the remote peer.
    pub label: String,
}

/// Network Controller State for outgoing connections.
#[derive(Debug, Default)]
pub struct OutgoingState {
    /// Current list of peer ids attempting to connect
    pub attempting: HashMap<Uuid, AddrInfo>,
    /// Current list of peers connected.
    pub connected: HashMap<Uuid, ConnInfo>,
}

/// Network Controller State for incoming connections.
#[derive(Debug, Default)]
pub struct IncomingState {
    /// Current list of peers connected.
    pub connected: HashMap<Uuid, ConnInfo>,
}

/// PeerData contains information to communicate with the peer.
#[derive(Debug)]
pub struct PeerData {
    /// transmit end of a channel to send commands to the peer.
    tx: Sender<Command>,
    /// handle on the peer's main loop.
    handle: JoinHandle<Result<(), peer::Error>>,
}

type PeerRepo = HashMap<Uuid, PeerData>;

/// NetworkController
/// The network controller holds state information about connections with peers, and configuration
/// The state information is split so that we can keep separate locks for areas that don't
/// overlap.
#[derive(Debug)]
pub struct NetworkController {
    // FIXME Replace id, label, addr with a single ConnInfo
    /// Unique id of the node in the network
    pub id: Uuid,
    /// A label to make it easy to read. Hopefully it is a unique string in the retwork,
    /// but it's really that distinguishes it.
    pub label: String,
    /// Address node is listening to for incoming request
    pub addr: SocketAddr,
    /// configuration. It is not protected by a mutex because it is read only.
    pub config: Arc<Controller>,
    /// peers contains information about each peer to allow the controller
    /// to communicate with the peer.
    /// * a tx end of a channel, where the peer constantly listens on the rx end.
    /// * a handle to the peer main loop. It is just used to terminate the peer
    ///   by calling abort on the handle.
    pub peers: Arc<Mutex<PeerRepo>>,
    /// Outgoing state
    pub outgoing: Arc<Mutex<OutgoingState>>,
    /// Incoming state
    pub incoming: Arc<Mutex<IncomingState>>,
    /// Idle state
    pub idle: Arc<Mutex<IdleState>>,
    /// Sending end of channel for peer -> controller
    /// A clone of tx is given to each peer so that they can communicate
    /// with the controller.
    pub tx_evt: Sender<Event>,
    /// Receiving end of channel for peer -> controller. The controller
    /// monitors this endpoint to learn about peer status.
    pub rx_evt: Receiver<Event>,
    /// Thread Handle for the listen thread.
    pub listen_handle: Option<JoinHandle<Result<(), Error>>>,
    /// Thread Handle for the monitor idle thread.
    pub monitor_idle_handle: Option<JoinHandle<()>>,
    /// Thread Handle for the monitor status thread.
    pub monitor_status_handle: Option<JoinHandle<()>>,
}

impl NetworkController {
    /// Create a new network controller
    pub fn new(label: String, config: Controller) -> Result<NetworkController, Error> {
        let addr =
            IpAddr::from_str(config.listen.addr.as_str()).map_err(|err| Error::InvalidAddr {
                source: err,
                detail: format!("Could not use {} as valid IP Address", config.listen.addr),
            })?;
        let addr = SocketAddr::from((addr, config.listen.port));

        let (tx_evt, rx_evt) = mpsc::channel(32); // FIXME 32 Automagick

        Ok(NetworkController {
            id: Uuid::new_v4(),
            label,
            addr,
            config: Arc::new(config),
            peers: Arc::new(Mutex::new(HashMap::new())),
            outgoing: Arc::new(Mutex::new(OutgoingState::default())),
            incoming: Arc::new(Mutex::new(IncomingState::default())),
            idle: Arc::new(Mutex::new(IdleState::default())),
            tx_evt,
            rx_evt,
            listen_handle: None,
            monitor_idle_handle: None,
            monitor_status_handle: None,
        })
    }

    /// This function is ran when we start the Network Controller.
    /// It looks at the network controller's configuration for an
    /// initial list of peers, and stores them in the
    /// controller's state.
    pub async fn initialize(&mut self) -> Result<(), Error> {
        // The user gave an initial list of peers to connect to in a file.
        // Here we identify the file, read the content (assumed to be an
        // array of addresses in JSON format).
        let working_dir = get_working_dir();
        // if the configuration gives an absolute path, push will replace the working dir.
        let mut path = PathBuf::from(&working_dir);
        path.push(&self.config.target.file);
        let content = fs::read_to_string(path).await.map_err(|err| Error::IO {
            source: err,
            detail: "Cannot read config file".to_owned(),
        })?;

        let addrs: Vec<String> =
            serde_json::from_str(&content).map_err(|err| Error::InvalidPeerFile { source: err })?;
        let addrs: HashSet<AddrInfo> = addrs.iter().try_fold(HashSet::new(), |mut acc, t| {
            let addr = SocketAddr::from_str(t).map_err(|err| Error::InvalidAddr {
                source: err,
                detail: format!("Could not turn {} into a network address", t),
            })?;
            acc.insert(AddrInfo {
                addr,
                attempt: Arc::new(Mutex::new(0)),
            });
            Ok(acc)
        })?;

        let idle_state = IdleState { addrs };

        let mut idles = self.idle.lock().await;
        *idles = idle_state;

        Ok(())
    }

    /// Spawn a thread to listen for incoming connection request from remote peer.
    async fn start_listen(&self) -> Result<JoinHandle<Result<(), Error>>, Error> {
        let controller = self.id;
        let label = self.label.clone();
        let tx_evt = self.tx_evt.clone();
        let addr = self.addr;
        let peers = self.peers.clone();
        let incoming = self.incoming.clone();
        let config = self.config.clone();
        let handle = tokio::spawn(async move {
            serve(controller, label, addr, tx_evt, peers, incoming, config).await
        });
        Ok(handle)
    }

    /// Spawn a thread which monitors a set of addresses.
    /// Every second, each address present in this set will be sent a request
    /// to connect.
    async fn start_monitor_idle(&self) -> Result<JoinHandle<()>, Error> {
        let controller = self.id;
        let label = self.label.clone();
        let controller_addr = self.addr;
        let tx_evt = self.tx_evt.clone();
        let idle = self.idle.clone();
        let peers = self.peers.clone();
        let outgoing = self.outgoing.clone();
        let config = self.config.clone();
        let handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1)); // Every second
            loop {
                // We wait for the periodic tick,
                interval.tick().await;

                let mut idle_guard = idle.lock().await;

                // We take the list of idle addresses, and for each address, we create a peer, start it, and
                // send it a request to connect to the given address. As we get new addr_info from
                // the stream, we build the next set of addr_info:
                // * If we cannot create a peer and send it the 'connect' command, then that
                // addr_info will figure in the new hashset.
                // * If on the other hand we can send a connect to a new peer, then we remove that
                // address from the hashset of idle addresses.
                let addrs = futures::stream::iter(&idle_guard.addrs)
                    .fold(HashSet::new(), |mut set, addr_info| {
                        let tx_evt = tx_evt.clone();
                        let peers = peers.clone();
                        let outgoing = outgoing.clone();
                        let config = config.clone();
                        let label = label.clone();
                        async move {
                            // If there are too many attempts at the moment, then we save that
                            // addr for the next round.
                            let mut outgoing = outgoing.lock().await;
                            if outgoing.attempting.len()
                                > config.outgoing.max_simultaneous_conn_attempts as usize
                            {
                                log::warn!(
                                    "Controller | Could not send 'connect' command for address {} | {}",
                                    addr_info.addr,
                                    "Too many simultaneous connection attempts"
                                );
                                set.insert(addr_info.clone());
                                return set;
                            }

                            let (tx_com, rx_com) = mpsc::channel(32);
                            let peer = Peer::new(
                                controller,
                                label,
                                controller_addr,
                                tx_evt,
                                tx_com.clone(),
                                rx_com,
                                config.peers.heartbeat_timeout,
                                config.peers.heartbeat_period,
                            );
                            let id = peer.id;
                            log::info!(
                                "Controller | Starting peer {}",
                                id.to_string().get(0..8).unwrap()
                            );
                            let handle = tokio::spawn(async move { peer.run().await });
                            let tx = tx_com.clone();
                            peers.lock().await.insert(
                                id,
                                PeerData {
                                    tx: tx.clone(),
                                    handle,
                                },
                            );
                            let attempt: i32 = *addr_info.attempt.lock().await;
                            let new_addr_info = AddrInfo {
                                addr: addr_info.addr,
                                attempt: Arc::new(Mutex::new(attempt + 1))
                            };
                            if let Err(err) =
                                send_connect(&id, new_addr_info.clone(), &tx).await
                            {
                                log::error!(
                                    "Controller | Error sending 'connect' command to peer {} | {err}",
                                    id.to_string().get(0..8).unwrap()
                                );
                                // FIXME Maybe we need to remove the data we just inserted from peers,
                                // otherwise it leaks.
                                set.insert(addr_info.clone());
                                set
                            } else {
                                // Everything is nominal, so we add the peer id and the address
                                // info.
                                //
                                // FIXME Need to see what's going on if the id was already in the
                                // map... but unlikely because we just created this uuid.
                                let _ = outgoing.attempting.insert(id, new_addr_info);
                                set
                            }
                        }
                    })
                    .await;

                idle_guard.addrs = addrs;
            }
        });
        Ok(handle)
    }

    /// Spawn a thread which monitors a set of addresses.
    /// Every second, each address present in this set will be sent a request
    /// to connect.
    async fn start_monitor_status(&self) -> Result<JoinHandle<()>, Error> {
        let controller = self.id;
        let label = self.label.clone();
        let controller_addr = self.addr;
        let outgoing = self.outgoing.clone();
        let incoming = self.incoming.clone();
        let handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5)); // Every second
            loop {
                // We wait for the periodic tick,
                interval.tick().await;

                let controller = ConnInfo {
                    id: controller,
                    label: label.clone(),
                    addr: controller_addr,
                };

                let incoming_guard = incoming.lock().await;

                let incoming = futures::stream::iter(&incoming_guard.connected)
                    .fold(Vec::new(), |mut acc, (_, info)| async move {
                        acc.push(info.clone());
                        acc
                    })
                    .await;

                let outgoing_guard = outgoing.lock().await;

                let outgoing = futures::stream::iter(&outgoing_guard.connected)
                    .fold(Vec::new(), |mut acc, (_, info)| async move {
                        acc.push(info.clone());
                        acc
                    })
                    .await;

                let summary = Summary {
                    controller,
                    incoming,
                    outgoing,
                };

                let output = serde_json::to_string(&summary).unwrap();

                log::info!("Status: {output}");
            }
        });
        Ok(handle)
    }

    /// The main network controller loop:
    /// We spawn a thread to listen to incoming tcp connection,
    /// We send a connect to all initial peers to connect to their remote,
    /// and then we just listen to incoming events.
    pub async fn run(&mut self) -> Result<(), Error> {
        // let tx_evt = self.tx_evt.clone();
        // let addr = self.addr;
        // let peers = self.peers.clone();
        // let incoming = self.incoming.clone();
        // let config = self.config.clone();
        // let handle =
        //     tokio::spawn(async move { serve(addr, tx_evt, peers, incoming, config).await });
        let handle = self.start_listen().await?;
        self.listen_handle = Some(handle);
        let handle = self.start_monitor_idle().await?;
        self.monitor_idle_handle = Some(handle);
        let handle = self.start_monitor_status().await?;
        self.monitor_status_handle = Some(handle);

        let peers = self.peers.clone();
        let outgoing = self.outgoing.clone();
        let incoming = self.incoming.clone();
        let idle = self.idle.clone();
        // let config = self.config.clone();
        // Start receiving events
        while let Some(event) = self.rx_evt.recv().await {
            match event {
                Event::BindError { source: _, addr } => {
                    log::error!("Network controller cannot bind to addr {}.", addr);
                    log::error!(
                        "Maybe modify the network.controller.listen section in the configuration."
                    );
                    log::error!("Terminating");
                    return Err(Error::Bind {
                        detail: format!("Network controller cannot bind to addr {}", addr),
                    });
                }
                Event::InvalidState {
                    id,
                    expected,
                    actual,
                } => {
                    // Note there is a slightly better solution, but requires nightly:
                    // expected .into_iter() .map(|s| s.to_string()) .intersperse(String::from(", ")) .collect(),
                    let expected = expected
                        .into_iter()
                        .fold(String::new(), |s, t| s + t.to_string().as_str() + " or ");
                    let expected = expected.trim_end_matches(" or ");
                    log::error!(
                        "Controller | Peer {} is not in its expected state | Actual {}. Expected {}",
                        id.to_string().get(0..8).unwrap(),
                        actual.to_string(),
                        expected // expected .into_iter() .map(|s| s.to_string()) .intersperse(String::from(", ")) .collect(),
                    );
                }
                Event::Connected { id } => {
                    // Peer is in OutConnecting, and has successfully connected, so we need to
                    // * send it the connection request command to initiate the handshake.
                    log::info!(
                        "Controller | Peer {} is connected.",
                        id.to_string().get(0..8).unwrap()
                    );
                    match peers.lock().await.get(&id).ok_or(Error::UnknownId {
                        id,
                        detail: "Controller | Could not find id in peer table.".to_owned(),
                    }) {
                        Err(err) => {
                            log::error!("{err}");
                        }
                        Ok(peer_data) => {
                            if let Err(err) = send_command_single_peer(
                                Command::SendConnRequest,
                                &peer_data.tx,
                                &id,
                            )
                            .await
                            {
                                log::error!(
                                    "Controller | Could not send 'connection request' to peer {} | {err}",
                                    id.to_string().get(0..8).unwrap());
                            }
                        }
                    };
                }
                Event::Listening { id } => {
                    // Peer is in InHandshaking, and is successfully listening to incoming
                    // messages, so we need to:
                    // * remove it from the list of attempted connections
                    // * add it to the list of actual connections.
                    // don't do anything else (wait for the remote peer to send handshake request.)
                    log::info!(
                        "Controller | Peer {} is listening.",
                        id.to_string().get(0..8).unwrap()
                    );
                    // FIXME What to do in that state
                    // let mut incoming_guard = incoming.lock().await;
                    // incoming_guard.connected.insert(id);
                }
                Event::OutAlive {
                    id,
                    peer_id,
                    peer_label,
                    peer_addr,
                } => {
                    log::info!("Controller | Connection with {} is live.", peer_label);
                    let mut outgoing_guard = outgoing.lock().await;
                    let _addr_info = outgoing_guard
                        .attempting
                        .remove(&id)
                        .expect("addr info for id");
                    outgoing_guard.connected.insert(
                        id,
                        ConnInfo {
                            addr: peer_addr,
                            id: peer_id,
                            label: peer_label,
                        },
                    );
                }
                Event::InAlive {
                    id,
                    peer_id,
                    peer_label,
                    peer_addr,
                } => {
                    log::info!("Controller | Connection with {} is live.", peer_label);
                    let mut incoming_guard = incoming.lock().await;
                    incoming_guard.connected.insert(
                        id,
                        ConnInfo {
                            addr: peer_addr,
                            id: peer_id,
                            label: peer_label,
                        },
                    );
                }
                Event::Disconnected { id, addr } => {
                    // We remove the id from the list of outgoing peers,
                    // and also push back the addr into the list of idle addresses.
                    log::info!(
                        "Controller | Peer {} is disconnected from {}.",
                        id.to_string().get(0..8).unwrap(),
                        addr
                    );
                    let addr_info = outgoing
                        .lock()
                        .await
                        .connected
                        .remove(&id)
                        .expect("addr_info for id");
                    let addr_info = AddrInfo {
                        addr: addr_info.addr,
                        attempt: Arc::new(Mutex::new(0)),
                    };
                    idle.lock().await.addrs.insert(addr_info);
                    let peer = peers.lock().await.remove(&id).expect("peer for id");
                    peer.handle.abort();
                }
                Event::Terminated { id } => {
                    log::info!(
                        "Controller | Peer {} is terminated.",
                        id.to_string().get(0..8).unwrap()
                    );
                    incoming.lock().await.connected.remove(&id);
                    let peer = peers.lock().await.remove(&id).expect("peer for id");
                    peer.handle.abort();
                }
                Event::ConnectionError { id, addr, source } => {
                    // The peer could not establish a Tcp connection.
                    // So remove it from the list of attempting, and put it back in the list of
                    // idle. The 'monitor_idle' thread will pick it up and automatically try to
                    // reconnect.
                    log::warn!(
                        "Controller | Peer {} cannot connect to {} | {source}",
                        id.to_string().get(0..8).unwrap(),
                        addr
                    );
                    let addr_info = outgoing
                        .lock()
                        .await
                        .attempting
                        .remove(&id)
                        .expect("addr_info for id");
                    idle.lock().await.addrs.insert(addr_info);
                }
            }
        }
        Ok(())
    }
}

/// Send a command
async fn send_command_single_peer(
    cmd: Command,
    tx: &Sender<Command>,
    id: &Uuid,
) -> Result<(), Error> {
    let cmd_id = cmd.to_string();
    if let Err(err) = tx.send(cmd).await {
        log::error!(
            "Controller | Could not send command {} to peer {} | {}",
            cmd_id,
            id.to_string().get(0..8).unwrap(),
            err
        );
        return Err(Error::CommandError {
            source: err,
            detail: "Network controller could not send command to peer. Receiver dropped"
                .to_owned(),
        });
        // TODO Handle bad communication with peer... probably restart it.
    }
    Ok(())
}

/// Send a request to connect to a peer identified by its id.
///
/// Pre requisite:
///   We have guarded against too many simultaneous connection attempts.
///
async fn send_connect(
    id: &Uuid,
    addr_info: AddrInfo,
    tx_com: &Sender<Command>,
) -> Result<(), Error> {
    if let Err(err) = tx_com
        .send(Command::Connect {
            addr: addr_info.addr,
            attempt: *addr_info.attempt.lock().await,
        })
        .await
    {
        log::error!(
            "Controller | Could not send connect command to peer {} | {}",
            id.to_string().get(0..8).unwrap(),
            err
        );
        return Err(Error::CommandError {
            source: err,
            detail: format!(
                "Controller | Could not send command to peer {} | Receiver dropped",
                id.to_string().get(0..8).unwrap()
            ),
        });
        // TODO Handle bad communication with peer... probably restart it.
    }
    Ok(())
}

/// Send a listen command to a single peer identified by its id.
async fn send_listen(
    stream: TcpStream,
    tx: &Sender<Command>,
    _incoming: Arc<Mutex<IncomingState>>,
    id: &Uuid,
    _config: Arc<Controller>,
) -> Result<(), Error> {
    // We need to send a 'listen' command to 'id'.
    // We need to guard against too many incoming connections.
    // let mut incoming = incoming.lock().await;
    // if incoming.attempting.len() == config.incoming.max_simultaneous_conn_attempts as usize {
    //     log::warn!(
    //         "Controller | Could not send listen command to peer {} | {}",
    //         id.to_string().get(0..8).unwrap(),
    //         "Too many simultaneous connection attempts"
    //     );
    //     return Err(Error::OutgoingConnAttemptLimit {
    //         detail: "connect".to_owned(),
    //     });
    // } else if let Err(err) = tx.send(Command::Listen { stream }).await {
    if let Err(err) = tx.send(Command::Listen { stream }).await {
        log::error!(
            "Controller | Could not send listen command to peer {} | {}",
            id.to_string().get(0..8).unwrap(),
            err
        );
        return Err(Error::CommandError {
            source: err,
            detail: format!(
                "Controller | Could not send command to peer {} | Receiver dropped",
                id.to_string().get(0..8).unwrap()
            ),
        });
        // TODO Handle bad communication with peer... probably restart it.
    } else {
        // If we have successfully sent a connect command, then we increment
        // the number of connection attempt count.
        // incoming.attempting.insert(*id);
    }
    Ok(())
}

/// This function is used by the network controller to listen to incoming
/// request from the network.
/// addr is the address we're listening on
/// tx is the channel through which we'll be sending network event back to
/// the controller's main loop.
async fn serve(
    controller: Uuid,
    label: String,
    addr: SocketAddr,
    tx: Sender<Event>,
    peers: Arc<Mutex<PeerRepo>>,
    incoming: Arc<Mutex<IncomingState>>,
    config: Arc<Controller>,
) -> Result<(), Error> {
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            let msg = Event::BindError { source: err, addr };
            if let Err(err) = tx.send(msg).await {
                return Err(Error::EventError {
                    source: err,
                    detail: "Controller | Could not send event to main loop | Receiver dropped"
                        .to_owned(),
                });
            } else {
                return Ok(());
            }
        }
    };

    log::info!("Controller | listening on {}.", addr);

    loop {
        let label = label.clone();
        match listener.accept().await {
            Ok((stream, _)) => {
                // We have received a connection, so:
                // 1. Create a Peer
                // 2. Spawn a thread for its main loop
                // 3. Send the peer a command to listen.
                let tx_event = tx.clone();
                let (tx_com, rx_com) = mpsc::channel(64); // FIXME Automagick
                let peer = Peer::new(
                    controller,
                    label,
                    addr,
                    tx_event,
                    tx_com.clone(),
                    rx_com,
                    config.peers.heartbeat_timeout,
                    config.peers.heartbeat_period,
                );
                let id = peer.id;
                let tx = tx_com.clone();
                let config = config.clone();
                let incoming = incoming.clone();
                let handle = tokio::spawn(async move { peer.run().await });
                peers.lock().await.insert(
                    id,
                    PeerData {
                        tx: tx.clone(),
                        handle,
                    },
                );
                if let Err(err) = send_listen(stream, &tx, incoming, &id, config).await {
                    log::error!("Could not send listen command {err}");
                }
            }
            Err(err) => {
                log::error!("Error accepting connection: {err:?}");
            }
        }
    }
}

/// Error type for NetworkController
#[derive(Debug)]
pub enum Error {
    /// Invalid socket address
    InvalidAddr {
        /// details
        source: AddrParseError,
        /// details
        detail: String,
    },
    /// Something is missing
    Bind {
        /// Error detail
        detail: String,
    },
    // /// Connect Errror
    // Connect {
    //     /// Source
    //     source: std::io::Error,
    //     /// Error detail
    //     detail: String,
    // },
    /// Too many simultaneous connection attempts
    OutgoingConnAttemptLimit {
        /// Error detail
        detail: String,
    },
    // /// Connect Errror
    // UnauthorizedConnectionAttempt {
    //     /// Error detail
    //     detail: String,
    // },
    /// An error occured while sending an event to the controller.
    /// Probably the receiver dropped.
    EventError {
        /// Source
        source: mpsc::error::SendError<Event>,
        /// Error detail
        detail: String,
    },
    /// An error occured while sending a command to the peer.
    /// Probably the receiver dropped.
    CommandError {
        /// Source
        source: mpsc::error::SendError<Command>,
        /// Error detail
        detail: String,
    },
    /// Looked for a Peer by id, could not find.
    UnknownId {
        /// Searched for id.
        id: Uuid,
        /// Error detail
        detail: String,
    },
    /// IO Error
    IO {
        /// Source
        source: std::io::Error,
        /// Error detail
        detail: String,
    },
    /// Content of the Peer File is invalid
    InvalidPeerFile {
        /// source error
        source: serde_json::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidAddr { source, detail } => {
                write!(
                    f,
                    "Invalid Network Address: {} (source: {})",
                    detail, source
                )
            }
            Error::EventError { source: _, detail } => {
                write!(f, "Could not send event to controller {}", detail)
            }
            Error::CommandError { source: _, detail } => {
                write!(f, "Could not send command to peer {}", detail)
            }
            Error::UnknownId { id, detail } => {
                write!(f, "Peer with id {} could not be found => {}", id, detail)
            }
            Error::Bind { detail } => {
                write!(f, "Cannot Bind Socket Address => {}", detail)
            }
            Error::OutgoingConnAttemptLimit { detail } => {
                write!(f, "Too many connection attempts => {}", detail)
            }
            Error::IO { source, detail } => {
                write!(f, "IO Error => {} [{}]", detail, source)
            }
            Error::InvalidPeerFile { source: _ } => {
                write!(f, "Invalid peer file content (Json Array of string)")
            }
        }
    }
}

/// A helper function to get the working directory
/// See https://github.com/jojolepro/amethyst-extra/blob/77acd8920f7b68494bddd538ac9946cb0e584d78/src/lib.rs#L526
pub fn get_working_dir() -> String {
    let mut base_path = String::from(
        std::env::current_exe()
            .expect("Failed to find executable path.")
            .parent()
            .expect("Failed to get parent directory of the executable.")
            .to_str()
            .unwrap(),
    );
    if base_path.contains("target/") || base_path.contains("target\\") {
        base_path = String::from(".");
    }
    base_path
}

/// Configuration for the network controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Controller {
    /// incoming section
    pub incoming: Incoming,
    /// outgoing section
    pub outgoing: Outgoing,
    /// peers section
    pub peers: Peers,
    /// listen section
    pub listen: Listen,
    /// target section
    pub target: Target,
}

/// Configuration for the network controller. Incoming section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incoming {
    /// maximum number of connections
    pub max_conn_count: i32,
    /// maximum number of simultaneous connection attempts
    pub max_simultaneous_conn_attempts: i32,
}

/// Configuration for the network controller. Outgoing section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outgoing {
    /// maximum number of simultaneous connection attempts
    pub max_simultaneous_conn_attempts: i32,
}

/// Configuration for the network controller. peers section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peers {
    /// maximum of connection attempt
    pub max_conn_attempt: i32,
    /// Number of seconds between connection attempts
    pub conn_attempt_delay: i32,
    /// maximum number of idle peers
    pub max_idle_count: i32,
    /// maximum number of banned peers
    pub max_banned_count: i32,
    /// heartbeat timeout (seconds)
    pub heartbeat_timeout: i32,
    /// heartbeat period (seconds)
    pub heartbeat_period: i32,
}

/// Configuration for the network controller. listen section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listen {
    /// Network address the network controller is listening on.
    pub addr: String,
    /// Port the network controller is listening on.
    pub port: u16,
}

/// Configuration for the network controller. target section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// path to a file containing an initial list of peer
    /// addresses to connect to.
    pub file: String,
}

/// summary
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// controller
    pub controller: ConnInfo,
    /// incoming
    pub incoming: Vec<ConnInfo>,
    /// outgoing
    pub outgoing: Vec<ConnInfo>,
}
