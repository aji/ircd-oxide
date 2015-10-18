% Oxen, the cluster protocol

Oxen is the server-to-server protocol used in `ircd-oxide`, and is designed to
provide reliable out-of-order delivered at-least-once messaging, and reliable
in-order delivered once messaging as a thin layer on top of this.

Because of `ircd-oxide`'s ability to compare and merge states, the out-of-order
delivered at-least-once messaging mode is good enough for managing state. For
cases that need to behave more closely to traditional IRC, such as `PRIVMSG`,
the in-order delivered once functionality can be used.

## High-level API

This section deals with the high-level API that Oxen presents to you as a
protocol user. The actual programming API, as Rust code, is not going to map
one-to-one with this. However, understanding the concrete API will be easier
with a good understanding of the high-level API it is designed to represent.

When interacting with Oxen as a user of the protocol, you will be expected to
deal with the following events:

  *  A message has arrived.
  *  A node is voluntarily leaving the cluster.
  *  A fresh node has joined the cluster.
  *  We give up and a node is not usefully reachable.
  *  A previously unreachable node is now reachable.

You are also allowed to make the following requests:

  *  Join a cluster, asking some server *S* for help.
  *  Start a cluster.
  *  Leave the cluster.
  *  Deliver a datagram to peer *P*.
  *  Broadcast a datagram.
  *  Deliver an in-order 1-1 datagram to peer *P*.
  *  Deliver an in-order broadcast datagram.

## The cluster

From a logical perspective, an Oxen cluster is a set of servers with no
inherent topology. It's acceptable to think of the cluster as a complete graph
of servers.

Each server has a unique ID, called the server ID, or SID, that is assigned by
the administrator. In the future, these may be generated from an acceptable
source of unique ID information, such as MAC addresses.

## Messaging

Oxen is built as a thin reliability layer on top of UDP. Oxen assigns each
outgoing message an ID and keeps track of outstanding messages. Upon receiving
a message, even one that has already been received, an acknowledgement is sent.
If no acknowledgement has been received for an outstanding message, the message
is resent.

## Last contact gossip

A table of last contacts is maintained, with a row and column for each server.
Random portions of this table are sent periodically to random peers in a gossip
protocol fashion. When a message is acknowledged, the timestamp for the
corresponding "contact" is the time the message was first sent. That is, if a
message is sent at times 1 and 2 and acknowledged at times 3 and 4, time 1 is
considered the contact time. Last contact times are kept up to date by
periodically sending no-op messages.

For some pair of servers, if the difference between the last contact and the
current time is above some threshold, then the corresponding link is considered
"possibly unusable". Otherwise, the link is considered "possibly usable".
Making any absolute claim about a link's usability is unwise, and so we
restrict ourselves to these two possibilities. There are valid reasons for an
otherwise reachable server to have "possibly unusable" links, such as firewall
rules. It's only when all links to a server are "possibly unusable" that the
server itself is considered "possibly unreachable".

Note that we do not need to try to find a possibly usable path between two
nodes by searching the graph with the last contacts table as an adjacency
matrix. If information about a link's usability is reaching us, it follows that
there must already be some path from us to the node providing that information.
Therefore, it's sufficient to look at only the last contact times for all links
toward a server.

## Dealing with possibly unusable links

If our link to a server is possibly unusable, but some possibly usable link to
that server exists elsewhere, we'd still like to be able to send messages to
that server. In this scenario, Oxen will send the message to some maybe usable
local link. The receiving node then repeats the process. If the target node is
truly reachable, the message will eventually arrive. To optimize this process,
Oxen may try to find a shortest path with possibly usable links and send the
message to the first server on that path.

## Giving up

If a peer has been in a possibly unreachable state for a certain amount of
time, Oxen may give up, mark the node as definitely unreachable, clear all
outstanding messages to that node, and drop any future messages to or from that
node. If the node becomes reachable again, Oxen will inform the protocol user
and resume normal operation.

Because of the nature of distributed systems, we *will* give up on a peer at a
different time than every other peer. In fact, we may not agree with other
peers on whether a node should be given up on at all. It's important that Oxen
remains consistent in the face of this.

Consider the scenario where we give up on a peer *P*. The protocol user is
informed, and we clear our queue, as if *P* has voluntarily left the cluster.
Suppose some other fully reachable peer *Q* has not yet given up on *P*. If *Q*
receives a message from *P* at this time, addressed to us, *Q* will happily
send it our way, and maybe eventually go back to considering *P* reachable. In
the meantime, we will receive a message from *P*, a server we considered dead!
In this scenario, we will simply drop the message from *P*. However, new
information from peers may reveal that *P* is reachable again. We revert *P*'s
status, informing the protocol user that *P* is now reachable, as if *P* has
voluntarily joined the cluster.

Protocol users should be careful when treating unexpected loss of contact with
servers significantly differently from expected loss of contact. A similar
warning applies to expected versus unexpected new contact.
