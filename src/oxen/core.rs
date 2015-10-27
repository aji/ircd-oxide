// oxen/core.rs -- the Oxen core
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The core Oxen logic

// Known problems:
//
//   o  Redelivery logic, all of it! If a host is unreachable and we have
//      pending messages to that host, we'll just keep retrying over and over
//      until the host becomes reachable again. There are some protocol things
//      that need to be figured out until this can be safely addressed, such as
//      how specifically to handle {dis,re}appearing hosts. I'm pretty sure I
//      want peers to request resynchronize in that scenario, but I don't want
//      to just assume that's how it will work at the moment.
//
//   o  Every use of HashMap, HashSet, and BinaryHeap, particularly keepalives.
//      While Rust doesn't itself have memory leaks, hash tables "can", in the
//      sense that we can put something in them that we never use again and
//      it would never be cleared. We should periodically garbage collect these,
//      for example clearing all keepalives above a certain threshold. For
//      example, it's not useful to keep around a pending keepalive that would
//      not cause the responding host to become reachable.
//
//   o  Embedded keepalives. The protocol as specified supports requesting and
//      responding to keepalives in regular parcels. Parts of the code are ready
//      to work with this, but there are still opportunities to integrate
//      embedded keepalives throughout the code. This optimization would only
//      save a small number of packets though (I think) so it's lower priority.
//
//   o  Handle Byzantine failure better (at all, even). Most of this code is
//      written with the assumption that peers are behaving correctly. For
//      example, we simply merge any incoming last contact table into our own.
//      A peer could send obscenely large values for the entire thing and
//      basically render the entire mechanism useless. A much more subtle
//      example is the potential leak in instances of Inbox where the next
//      expected packet is never received and we continue to collect later
//      packets forever. A bug that prevents any packet from being redelivered
//      would cause this to happen, which is a pretty significant bug overall.
//
// This is by no means a complete list! I've included it as a reminder of the
// work left to make Oxen a solid solution. Much of what ircd-oxide claims to be
// rests on the abstractions that Oxen provides being airtight. If Oxen cannot
// handle failure modes gracefully, then the rest of the IRCD cannot hope to.
// I've spent a lot of the past week rushing a framework to allow further growth
// and an implementation that will work well enough to support oxide development
// elsewhere, but I've cut a lot of corners in the process and am simply noting
// some of the harder problems that I've glossed over in the name of simplicity
// so that they're not missed when I come back for round 2.

use rand::random;
use std::cmp;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::From;
use std::marker::PhantomData;
use time::{Duration, Timespec};

use oxen::data::*;
use oxen::lc::LastContact;
use util::Sid;
use xenc;
use xenc::FromXenc;

pub type Timer = u64;

pub struct Oxen {
    peers: HashSet<Sid>,

    lc: LastContact,
    peer_status: HashMap<Sid, PeerStatus>,

    pending_ka: HashMap<(Sid, KeepaliveId), Timespec>,
    pending_msgs: HashMap<(Sid, MsgId), PendingMessage>,
    pending_msg_timers: HashMap<Timer, (Sid, MsgId)>,

    brd_seq: SeqNum,
    one_seq: HashMap<Sid, SeqNum>,

    brd_inbox: Inboxes<Broadcast>,
    one_inbox: Inboxes<OneToOne>,

    gossip_timer: Timer,
    lc_timer: Timer,
}

pub enum OxenEvent {
    Message(Sid, Vec<u8>),
    PeerVisible(Sid),
    PeerVanished(Sid),
}

pub trait OxenHandler {
    fn now(&self) -> Timespec;

    fn me(&self) -> Sid;

    fn queue_send<X>(&mut self, peer: Sid, data: X)
    where xenc::Value: From<X>;

    fn timer_set(&mut self, at: Duration) -> Timer;

    fn timer_cancel(&mut self, timer: Timer);
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PeerStatus {
    Unchecked,
    Available,
    Unavailable,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct PendingMessage {
    to: Sid,
    id: MsgId,
    redeliver: Timer,
    interval: Duration,
    msg: MsgData,
}

impl Oxen {
    pub fn new<H>(hdlr: &mut H) -> Oxen
    where H: OxenHandler {
        let mut oxen = Oxen {
            peers: HashSet::new(),

            lc: LastContact::new(hdlr.me()),
            peer_status: HashMap::new(),

            pending_ka: HashMap::new(),
            pending_msgs: HashMap::new(),
            pending_msg_timers: HashMap::new(),

            brd_seq: random(),
            one_seq: HashMap::new(),

            brd_inbox: Inboxes::new(),
            one_inbox: Inboxes::new(),

            gossip_timer: 0,
            lc_timer: 0,
        };

        oxen.peers.insert(hdlr.me());

        // start these timers
        oxen.last_contact_gossip(hdlr);
        oxen.check_last_contact(hdlr);

        oxen
    }

    pub fn add_peer<H>(&mut self, hdlr: &mut H, sid: Sid)
    where H: OxenHandler {
        if sid == hdlr.me() {
            return;
        }

        self.peers.insert(sid);
        self.peer_status.insert(sid, PeerStatus::Unchecked);

        let brd_seq = self.brd_seq;
        let one_seq = *self.one_seq.entry(sid).or_insert_with(|| random());

        info!("synchronizing with {}", sid);
        self.send_with_redelivery(hdlr, &sid, MsgDataBody::MsgSync(MsgSync {
            brd: brd_seq,
            one: one_seq,
        }));
    }

    pub fn incoming<H>(&mut self, hdlr: &mut H, from: Sid, data: xenc::Value)
    where H: OxenHandler {
        let p = match Parcel::from_xenc(data) {
            Ok(p) => p,
            Err(_) => {
                error!("could not decode a Parcel from incoming XENC");
                return;
            },
        };

        if let Some(ka) = p.ka_rq {
            debug!("responding to {} keepalive {}", from, ka);
            hdlr.queue_send(from, Parcel {
                ka_rq: None,
                ka_ok: Some(ka),
                body: ParcelBody::Missing,
            });
        }

        if let Some(kk) = p.ka_ok {
            debug!("received keepalive {} ok from {}", kk, from);

            match self.pending_ka.remove(&(from, kk)) {
                Some(at) => self.lc.put(hdlr.me(), from, at),
                _ => info!("stray keepalive {} from {}", kk, from),
            }
        }

        match p.body {
            ParcelBody::Missing => { },
            ParcelBody::MsgData(data) => self.handle_msg_data(hdlr, data),
            ParcelBody::MsgAck(data) => self.handle_msg_ack(hdlr, data),
            ParcelBody::LcGossip(data) => self.handle_lc_gossip(hdlr, data),
        }
    }

    pub fn timeout<H>(&mut self, hdlr: &mut H, timer: Timer)
    where H: OxenHandler {
        if timer == self.lc_timer {
            self.check_last_contact(hdlr);
            return;
        }

        if timer == self.gossip_timer {
            self.last_contact_gossip(hdlr);
            return;
        }

        match self.pending_msg_timers.remove(&timer) {
            Some(k) => match self.pending_msgs.remove(&k) {
                Some(pending) => self.redeliver(hdlr, pending),
                _ => error!("inconsistency in pending message tables!"),
            },
            _ => error!("unknown timer has fired!"),
        };
    }

    pub fn send_broadcast<H>(&mut self, hdlr: &mut H, data: Vec<u8>)
    where H: OxenHandler {
        let peers: Vec<Sid> = self.peers.iter().cloned().collect();

        let brd_seq = self.brd_seq.wrapping_add(1);
        self.brd_seq = brd_seq;

        for p in peers {
            if p == hdlr.me() {
                continue;
            }

            self.send_with_redelivery(hdlr, &p, MsgDataBody::MsgBrd(MsgBrd {
                seq: brd_seq,
                data: data.clone(),
            }));
        }
    }

    pub fn send_one<H>(&mut self, hdlr: &mut H, to: Sid, data: Vec<u8>)
    where H: OxenHandler {
        if to == hdlr.me() {
            error!("tried to send a one-to-one message to ourself! dropping.");
            return;
        }

        let one_seq = match self.one_seq.get_mut(&to) {
            Some(seq) => { *seq = seq.wrapping_add(1); *seq },
            None => {
                error!("tried to send a message to a non-synced node! dropping.");
                return;
            },
        };

        self.send_with_redelivery(hdlr, &to, MsgDataBody::MsgOne(MsgOne {
            seq: one_seq,
            data: data,
        }));
    }

    fn redeliver<H>(&mut self, hdlr: &mut H, mut pending: PendingMessage)
    where H: OxenHandler {
        let key = (pending.to, pending.id);

        // TODO: fail after a number of retries
        // TODO: retry in longer intervals

        debug!("redelivering {} to {}",
            match pending.msg.body {
                MsgDataBody::Missing => "nothing?",
                MsgDataBody::MsgSync(_) => "syn",
                MsgDataBody::MsgFinal(_) => "fin",
                MsgDataBody::MsgBrd(_) => "brd",
                MsgDataBody::MsgOne(_) => "one",
            },
            pending.to
        );

        pending.redeliver = hdlr.timer_set(pending.interval);

        // TODO: check this return value
        self.routed(hdlr, &pending.to, Parcel {
            ka_rq: None,
            ka_ok: None,
            body: ParcelBody::MsgData(pending.msg.clone()),
        });

        self.pending_msg_timers.insert(pending.redeliver, key);
        self.pending_msgs.insert(key, pending);
    }

    fn send_with_redelivery<H>(&mut self, hdlr: &mut H, to: &Sid, m: MsgDataBody)
    where H: OxenHandler {
        if *to == hdlr.me() {
            error!("oxen tried to send a message to itself! dropping.");
            return;
        }

        let ival = Duration::milliseconds(800);

        let id = random();

        let msg = MsgData {
            to: *to,
            fr: hdlr.me(),
            id: Some(id),
            body: m,
        };

        let pending = PendingMessage {
            to: *to,
            id: id,
            redeliver: hdlr.timer_set(ival),
            interval: ival,
            msg: msg.clone(),
        };

        // TODO: check this return value
        self.routed(hdlr, to, Parcel {
            ka_rq: None,
            ka_ok: None,
            body: ParcelBody::MsgData(msg),
        });

        self.pending_msg_timers.insert(pending.redeliver, (*to, id));
        self.pending_msgs.insert((*to, id), pending);
    }

    fn routed<H, X>(&mut self, hdlr: &mut H, to: &Sid, data: X) -> bool
    where H: OxenHandler, xenc::Value: From<X> {
        if *to == hdlr.me() {
            error!("oxen tried to send a routed message to itself! dropping.");
            return false;
        }

        let thresh = Duration::seconds(20);

        let route = match self.lc.route(to, hdlr.now(), thresh) {
            Some(r) => r,
            None => return false,
        };

        hdlr.queue_send(route, data);

        true
    }

    fn check_last_contact<H>(&mut self, hdlr: &mut H)
    where H: OxenHandler {
        self.lc_timer = hdlr.timer_set(Duration::milliseconds(1000));

        for p in self.peers.iter() {
            if *p == hdlr.me() {
                continue;
            }

            let lc = self.lc.get(&hdlr.me(), p);
            let age = (hdlr.now() - lc).num_seconds();

            if age >= 2 {
                debug!("sending keepalive to {}", p);
                let ka = random();
                self.pending_ka.insert((*p, ka), hdlr.now());
                hdlr.queue_send(*p, Parcel {
                    ka_rq: Some(ka),
                    ka_ok: None,
                    body: ParcelBody::Missing,
                });
            }

            let reachable = self.lc
                .reachable(p, hdlr.now(), Duration::seconds(20));

            let status = self.peer_status
                .entry(*p)
                .or_insert(PeerStatus::Unchecked);

            match *status {
                PeerStatus::Unchecked => {
                    if reachable {
                        info!("promoting {} out of unchecked", p);
                        *status = PeerStatus::Available;
                    }
                },

                PeerStatus::Available => {
                    if !reachable {
                        info!("{} has become unavailable", p);
                        *status = PeerStatus::Unavailable;
                    }
                },

                PeerStatus::Unavailable => {
                    if reachable {
                        info!("{} is available again", p);
                        *status = PeerStatus::Available;
                    }
                },
            }
        }
    }

    fn handle_msg_data<H>(&mut self, hdlr: &mut H, data: MsgData)
    where H: OxenHandler {
        // simply forward the message if not to me

        if data.to != hdlr.me() {
            let to = data.to;
            self.routed(hdlr, &to, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgData(data),
            });
            return;
        }

        // otherwise, acknowledge it and continue handling

        if let Some(id) = data.id {
            let parcel = Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgAck(MsgAck {
                    to: data.fr,
                    fr: hdlr.me(),
                    id: id,
                })
            };
            self.routed(hdlr, &data.fr, parcel);
        }

        match data.body {
            MsgDataBody::MsgSync(syn) => {
                info!("got synchronization from {}", data.fr);

                // TODO: check for logic errors
                self.brd_inbox.get_mut(data.fr).synchronize(syn.brd);
                self.one_inbox.get_mut(data.fr).synchronize(syn.one);
            },

            MsgDataBody::MsgBrd(brd) => {
                self.brd_inbox.get_mut(data.fr).incoming(brd.seq, brd.data, |_| ());
            },

            MsgDataBody::MsgOne(one) => {
                self.one_inbox.get_mut(data.fr).incoming(one.seq, one.data, |_| ());
            },

            _ => { },
        }
    }

    fn handle_msg_ack<H>(&mut self, hdlr: &mut H, data: MsgAck)
    where H: OxenHandler {
        // simply forward the acknowledgement if not to me

        if data.to != hdlr.me() {
            let to = data.to;
            self.routed(hdlr, &to, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgAck(data),
            });
            return;
        }

        // otherwise, handle it

        if let Some(pending) = self.pending_msgs.remove(&(data.fr, data.id)) {
            hdlr.timer_cancel(pending.redeliver);
            self.pending_msg_timers.remove(&pending.redeliver);
        }
    }

    fn last_contact_gossip<H>(&mut self, hdlr: &mut H)
    where H: OxenHandler {
        self.gossip_timer = hdlr.timer_set(Duration::milliseconds(1000));

        let gossip = self.make_gossip();

        for p in self.peers.iter() {
            if *p == hdlr.me() {
                continue;
            }

            hdlr.queue_send(*p, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::LcGossip(gossip.clone()),
            });
        }
    }

    fn handle_lc_gossip<H>(&mut self, hdlr: &mut H, data: LcGossip)
    where H: OxenHandler {
        for (from, times) in data.rows.into_iter() {
            for (to, at) in data.cols.iter().zip(times.into_iter()) {
                self.lc.put(from, *to, at);
            }
        }
    }

    fn make_gossip(&self) -> LcGossip {
        let cols: Vec<Sid> = self.peers.iter().cloned().collect();

        let mut rows = HashMap::new();

        for p in self.peers.iter() {
            rows.insert(*p, cols.iter().map(|q| self.lc.get(p, q)).collect());
        }

        LcGossip {
            rows: rows,
            cols: cols,
        }
    }
}

struct Inbox<Kind: 'static> {
    synchronized: bool,
    next_seq: SeqNum,
    pending: BinaryHeap<InboxPending>,
    _kind: PhantomData<&'static mut Kind>,
}

struct Broadcast;
struct OneToOne;

struct InboxPending {
    seq: SeqNum,
    data: Vec<u8>,
}

impl PartialOrd for InboxPending {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.seq.partial_cmp(&other.seq).map(|o| o.reverse())
    }
}

impl Ord for InboxPending {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.seq.cmp(&other.seq).reverse()
    }
}

impl PartialEq for InboxPending {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq
    }
}

impl Eq for InboxPending { }

struct Inboxes<Kind: 'static> {
    map: HashMap<Sid, Inbox<Kind>>,
}

impl<Kind> Inbox<Kind> {
    fn new() -> Inbox<Kind> {
        Inbox {
            synchronized: false,
            next_seq: 0,
            pending: BinaryHeap::new(),
            _kind: PhantomData
        }
    }

    fn synchronized(&self) -> bool {
        self.synchronized
    }

    fn synchronize(&mut self, seq: SeqNum) {
        self.synchronized = true;
        self.next_seq = seq + 1;
    }

    fn incoming<F>(&mut self, seq: SeqNum, data: Vec<u8>, mut deliver: F)
    where F: FnMut(Vec<u8>) {
        self.pending.push(InboxPending { seq: seq, data: data });

        loop {
            let eat = match self.pending.peek() {
                Some(m) => m.seq <= self.next_seq,
                _ => false,
            };

            if !eat {
                return;
            }

            let next_seq = if let Some(m) = self.pending.pop() {
                if m.seq < self.next_seq {
                    self.next_seq
                } else {
                    deliver(m.data);
                    self.next_seq.wrapping_add(1)
                }
            } else {
                error!("logic error: good peek followed by bad pop?");
                return;
            };

            self.next_seq = next_seq;
        }
    }
}

impl<Kind> Inboxes<Kind> {
    fn new() -> Inboxes<Kind> {
        Inboxes {
            map: HashMap::new(),
        }
    }

    fn get_mut(&mut self, sid: Sid) -> &mut Inbox<Kind> {
        self.map.entry(sid).or_insert_with(|| Inbox::new())
    }
}

#[test]
fn test_inbox_easy() {
    let mut inbox: Inbox<Broadcast> = Inbox::new();

    inbox.synchronize(99);
    inbox.incoming(100, b"a".to_vec(), |v| assert!(v[0] == b'a'));
    inbox.incoming(101, b"b".to_vec(), |v| assert!(v[0] == b'b'));
    inbox.incoming(102, b"c".to_vec(), |v| assert!(v[0] == b'c'));
    inbox.incoming(103, b"d".to_vec(), |v| assert!(v[0] == b'd'));
}

#[test]
fn test_inbox_backwards() {
    let mut inbox: Inbox<Broadcast> = Inbox::new();
    let mut received = Vec::new();

    inbox.synchronize(99);
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));
    inbox.incoming(102, b"c".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));

    println!("{:?}", received);
    assert!(received == b"abcd");
}

#[test]
fn test_inbox_duplicates() {
    let mut inbox: Inbox<Broadcast> = Inbox::new();
    let mut received = Vec::new();

    inbox.synchronize(99);
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(102, b"c".to_vec(), |v| received.push(v[0]));
    inbox.incoming(102, b"c".to_vec(), |v| received.push(v[0]));
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));

    println!("{:?}", received);
    assert!(received == b"abcd");
}

#[test]
fn test_inbox_mishmash() {
    let mut inbox: Inbox<Broadcast> = Inbox::new();
    let mut received = Vec::new();

    inbox.synchronize(99);
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));
    inbox.incoming(102, b"c".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.incoming(102, b"c".to_vec(), |v| received.push(v[0]));
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));

    println!("{:?}", received);
    assert!(received == b"abcd");
}
