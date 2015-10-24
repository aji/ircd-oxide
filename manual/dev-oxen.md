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

## Concepts

***The cluster.*** From a logical perspective, an Oxen cluster is a set of
servers with no inherent topology. It's acceptable to think of the cluster as a
complete graph of servers. Each server has a unique ID, called the server ID,
or SID, that is assigned by the administrator. In the future, these may be
generated from an acceptable source of unique ID information, such as MAC
addresses.

***Messaging.*** Oxen is built as a thin reliability layer on top of UDP. Oxen
assigns each outgoing message an ID and keeps track of outstanding messages.
Upon receiving a message, even one that has already been received, an
acknowledgement is sent. If no acknowledgement has been received for an
outstanding message, the message is resent.

***Last contact table.*** Each node keeps a table of last contacts, with a row
and column for each server. Random portions of this table are sent periodically
to random peers in a gossip protocol fashion. When a message is acknowledged,
the timestamp for the corresponding "contact" is the time the message was first
sent. That is, if a message is sent at times 1 and 2 and acknowledged at times
3 and 4, time 1 is considered the contact time. Last contact times are kept up
to date by periodically sending no-op messages.

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

## Handling failure

If our link to a server is possibly unusable, but some possibly usable link to
that server exists elsewhere, we'd still like to be able to send messages to
that server. In this scenario, Oxen will send the message to some maybe usable
local link. The receiving node then repeats the process. If the target node is
truly reachable, the message will eventually arrive. To optimize this process,
Oxen may try to find a shortest path with possibly usable links and send the
message to the first server on that path.

***When to give up.*** If a peer has been in a possibly unreachable state for a
certain amount of time, Oxen may give up, mark the node as definitely
unreachable, clear all outstanding messages to that node, and drop any future
messages to or from that node. If the node becomes reachable again, Oxen will
inform the protocol user and resume normal operation.

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

## Parcel schema

The term *parcel* is used as it's distinct from commonly used terms like
*packet* and *message*, but similar enough that the meaning is clear. Oxen
nodes communicate by exchanging parcels. Note that any node can send a parcel
to any other node at any time, whether the receiving node is "aware" of the
sending node or not, and vice versa.

At the top level, a parcel is encoded as an XENC dictionary, with the following
fields:

 * `ka`: The keepalive ID to respond with (optional).
 * `kk`: The keepalive ID being responded to (optional).

Up to one of the following is used as a key-value pair representing the body of
the parcel:

### `md`: Message data

The value part of this body is a dictionary with the following keys:

 * `to`: The SID this parcel is intended for.
 * `fr`: The SID that generated this message.
 * `id`: The unique ID of this message (optional).
 * `d`: Message data

Forwarding is implied if `to` is not the SID of the receiving node. If `id` is
omitted, no acknowledgement is requested.

### `ma`: Message acknowledge

The value part of this body is a dictionary with the following keys:

 * `to`: The SID whose message is being acknowledged.
 * `fr`: The SID that is acknowledging successful delivery.
 * `id`: The ID of the message being acknowledged.

Forwarding is implied if `to` is not the SID of the receiving node.

### `lc`: Last contact gossip

The value part of this body is a dictionary with the following keys:

 * `lc`: A dictionary of SID&rarr;row pairs.
 * `p`: The list of SIDs corresponding to columns.

As an example, consider the following parcel body.

```text
{
  lc: {
    AAA: [5, 3, 1, 9],
    BBB: [6, 7, 7, 8],
    DDD: [9, 8, 1, 6]
  },
  p: ["AAA", "BBB", "CCC", "EEE"]
}
```

This would correspond to the following last contact table fragment:

|         | AAA | BBB | CCC | DDD | EEE |
|---------|:---:|:---:|:---:|:---:|:---:|
| **AAA** |  5  |  3  |  1  |  -  |  9  |
| **BBB** |  6  |  7  |  7  |  -  |  8  |
| **CCC** |  -  |  -  |  -  |  -  |  -  |
| **DDD** |  9  |  8  |  1  |  -  |  6  |
| **EEE** |  -  |  -  |  -  |  -  |  -  |

### Keepalives

The `ka` and `kk` fields are used for simple parcel acknowledgement for keeping
the last contact table fresh. Keepalives are the *only* way to update the last
contact table. As mentioned previously, the resulting last contact entry is
updated to the time the parcel was originally sent.

Keepalive functionality exists orthogonally to parcel body functionality, but
is included in parcels to reduce the number of messages sent, i.e. if I want to
send a message to a peer, ask them for a keepalive, and respond to a keepalive
request of theirs, I can do all three in a single packet. We'll walk through,
with packet data examples, a scenario where *A* sends a message to *B* via *P*
(presumably *A* and *B* cannot communicate directly):

__*A* sends *P* a parcel with `ka` 123 and `md` addressed to *B* from *A*.__

```text
A to P: {
  ka: 123,
  md: { to: "B", fr: "A", id: 9999, d: ...data... }
}
```

__*P* receives the parcel and sends *A* a parcel with `kk` 123, and *B* a
parcel with `ka` 456 and `md` addressed to *B* from *A*__

```text
P to A: {
  kk: 123
}
```

```text
P to B: {
  ka: 456
  md: { to: "B", fr: "A", id: 9999, d: ...data... }
}
```

__*B* receives the parcel and sends *P* a parcel with `kk` 456, `ka` 345, and
`ma` addressed to *A* from *B*.__

```text
B to P: {
  kk: 456
  ka: 345
  ma: { to: "A", fr: "B", id: 999 }
}
```

__*P* receives the parcel and sends *A* a parcel with `ka` 789 and `ma`
addressed to *A* from *B*, and sends *B* a parcel with `kk` 345.__

```text
P to A: {
  ka: 789
  ma: { to: "A", fr: "B", id: 999 }
}
```

```text
P to B: {
  kk: 345
}
```

__*A* receives the parcel and sends *P* a parcel wtih `kk` 789.__

```text
A to P: {
  kk: 789
}
```

__Conclusion.__ A total of 7 messages have been sent to establish last contact
figures in both directions for 2 links between 3 hosts, and to deliver a
message with acknowledgement totaling 4 hops.

## Message data (`md`) body types.

The body of a message data (`md`) parcel carries further protocol meaning, and
can take on any of the following:

### Synchronize message

```text
{
  m: "s",
  b: 123,
  1: 345
}
```

Used by the sending node to pick starting broadcast and one-to-one sequence
numbers for the sending node in the receiving node's buffers. Messages received
before synchronization are errors. Synchronization messages received after any
regular message are errors. (In other words, the only additional
synchronization messages a node is expected to receive are redeliveries of the
first synchronization message.)

The sequence numbers are one less than the next message to be delivered. That
is, if *A*'s broadcast sequence numbers is synchronized to 35 and broadcast
messages with sequence numbers 34, 35, 36, and 37 arrive, the receiving node
should only deliver 36 and 37.

### Finalize message

```text
{
  m: "f"
  b: 678,
  1: 789
}
```

Used by the sending node to indicate that no further messages will be sent to
the receiving node. There are no half-open associations in Oxen, so the
receiving node should not send any more messages either. The `b` and `1` fields
are sequence numbers for the last broadcast and one-to-one messages the sending
node has sent. As an example, if a node finalizes broadcast sequence number 97
and the receiving node has not yet received messages 94, 96, and 97, it should
wait for those messages to be re-sent before forgetting the sending node
completely.

### Broadcast message

```text
{
  m: "b",
  s: 123,
  d: "...data..."
}
```

A message to be added to the sending node's broadcast buffer. `s` is the
sequence number of this broadcast.

### One to one message

```text
{
  m: "1",
  s: 345,
  d: "...data..."
}
```

A message to be added to the one-to-one buffer for the sending node, on the
receiving node. `s` is the sequence number of this message.
