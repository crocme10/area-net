# Peer

A peer is a component in the node responsible for communication with
a remote node.

## Components

### Identity

A peer contains several pieces of data to identify itself:
- A unique id to distinguish that peer from other peers. This is often
  used by the controller as an index in a map.
- The controller's id, which is unique in the peer-to-peer network.
- A label, which is hopefully unique in the peer-to-peer network, and is
  used to have a readable handle to refer to that node.
- The controller's network address.

### State

A peer follows a (complex) state machine driven by commands issued by
the controller, or by other peer's thread. So the peer has a PeerState

### Remote communication

The peer communicates with remote peers using a 
[codec](https://docs.rs/tokio-util/latest/tokio_util/codec/index.html).
It keeps the sink, so that the main loop can send messages to the remote.
The stream is given to a detached thread, which listen to incoming messages

### Internal communication

The peer has 3 channel handles:
* A command receiver, which is monitored by the main loop.
* An event transmitter, to send events to the controller.
* Several event transmitters, so that several threads responsible for
  the peer's behavior can send information back to the peer's main loop.

### Housekeeping

Some configuration settings, and handles to several threads.

## Behavior

Currently the peer has several threads

1. The main loop, handles commands
2. The 'listen loop', listens for messages from the codec's stream.
3. The 'heartbeat loop', which periodically a command to the main loop
   to send a 'heartbeat request' to the remote peer.
4. Whenever the main loop sends a 'heartbeat request', it creates a 
   separate thread, the 'heartbeat timeout', which sleeps for the
   duration of the timeout, and then sends a 'heartbeat timeout'
   command to the main loop, to indicate that no 'heartbeat response'
   has been received before the timeout expires. The 'heartbeat
   response' will abort this thread usually before the timeout.
 

