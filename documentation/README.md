# Documentation

## Design

The node that constitute this peer-to-peer network are made of a [Network Controller](./controller.md)
coordinating [Peers](./peer.md) that handle the TCP Connection between peers.

When a peer wants to send a message to a remote, this is done using an encoder, which translates the message
into frames, and then each frame is sent over the wire.

## Communication

Peer-to-peer interactions can be broken into 3 groups:

### Handshake

### Heartbeat

Heartbeat provides a way for a peer to detect if the connection to the remote is available. It is
also a way to estimate the Round Trip Time for message transmission.

![Sequence Diagram](/assets/heartbeat-sequence.svg)

### Discovery

The sequence of messages between the actors involved in the network discovery:

![Sequence Diagram](/assets/network-discovery-sequence.svg)

More about [Network Discovery](./network-discovery.md).

## Codec
