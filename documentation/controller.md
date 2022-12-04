# Network controller

This peer-to-peer network is similar to a graph, with nodes communicating with
each other over TCP. It is a directed graph, where an edge is a TCP connection,
with the tail beeing the node issuing the connection, and the head the node
accepting the connection. The tail issues requests, and the head sends
responses back.

A node has two components:

1. The (unique) network controller
2. Several peers, one per connection to remote nodes, either incoming or outgoing
   connections.

The network controller (aka Controller) is responsible for:

* Coordinating and managing peers
* Maintaining the health of the node

## Behavior

Currently the Controller has 6 threads:

1. The main loop, handles events coming from a
   [mpsc channel](https://docs.rs/tokio/latest/tokio/sync/mpsc/fn.channel.html)
2. The 'listen loop', listens for incoming connections from remote peers. When a
   new connection is accepted, this listen loop creates a new peer, and spawns a
   detached thread for the execution of this peer's main loop. All communication
   with the remote is handled by the peer. The peer is given a transmitter, so
   that it can communicate with the controller by sending events. Conversely, the
   controller creates a channel for that peer, and keeps a transmitter. The
   controller can then issue commands to the peer.
3. The 'monitor idle', runs periodically. It analyzes a list of network addresses,
   and if conditions are met, it creates a new peer, and also spawn a detached
   thread for the execution of this peer's main loop. The difference with the 
   'listen loop' is that the 'monitor idle' sends the peer a 'connect' command
   to effectively establish a connection with the remote.
4. The 'monitor status', runs also periodically. It analyzes some Controller's 
   data structures, and produces a report, which can be dumped.
5. The 'network discovery' loop, runs periodically, and is responsible for
   broadcasting messages to remote peers to build a consensus about the state of
   the network as a whole.
