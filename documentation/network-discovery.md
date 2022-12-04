# Network discovery

One of the goal of this peer-to-peer network is for any node to become aware of the whole network.
Of course this implies the network's size is limited, otherwise the discovery would consume too
much resources.

Any node should learn about
* All the other nodes
* All the other connections between these nodes, which includes their direction and the connection's rtt.
This knowledge represents a directed graph, which could possibly cyclic.

At any point in time there is a unique true network graph that accurately describes the network. These
nodes have to come to an agreement (a consensus) about this graph. But information about the network takes
time to travel, and the network also changes constantly. So a node can only approach the true network graph,
this is eventual consistency.

## Network representation

The network can be represented as a graph with a set of nodes and directed edges:

Each node contains some information about the network controller, such as id, label, network address.

Each edge (directed, from connector to listener) can have the round trip time attribute.

Note that the graph can be cyclic.

Graphs are notably difficult to represent in rust, precisely because of this cyclic nature which interferes
with rust's ownership rules.

## Network discovery

Each node must store a representation of the network (a graph, as described above).

Out node send this graph in a 'ContactsRequest' message.

The receiving node must then fuse this incoming graph with his own, keeping only the most
relevant information. It then sends this new graph back the original node in a 'ContactsResponse' message.
