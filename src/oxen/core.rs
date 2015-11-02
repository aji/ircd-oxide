// oxen/core.rs -- the Oxen core
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The core Oxen logic
//!
//! Known problems:
//!
//!   o  Redelivery logic, all of it! If a host is unreachable and we have
//!      pending messages to that host, we'll just keep retrying over and over
//!      until the host becomes reachable again. There are some protocol things
//!      that need to be figured out until this can be safely addressed, such as
//!      how specifically to handle {dis,re}appearing hosts. I'm pretty sure I
//!      want peers to request resynchronize in that scenario, but I don't want
//!      to just assume that's how it will work at the moment.
//!
//!   o  Every use of HashMap, HashSet, and BinaryHeap. While Rust doesn't
//!      itself have real memory leaks, hash tables "can", in the sense that we
//!      can put something in them that we never use again and it would never be
//!      cleared.  We should be periodically garbage collecting these.
//!
//!   o  Embedded keepalives. The protocol as specified supports requesting and
//!      responding to keepalives in regular parcels. Parts of the code are
//!      ready to work with this, but there are still opportunities to integrate
//!      embedded keepalives throughout the code. This optimization would only
//!      save a small number of packets though (I think) so it's lower priority.
//!
//!   o  Handle Byzantine failure better (at all, even). Most of this code is
//!      written with the assumption that peers are behaving correctly. For
//!      example, we simply merge any incoming last contact table into our own.
//!      A peer could send obscenely large values for the entire thing and
//!      basically render the entire mechanism useless. A much more subtle
//!      example is the potential leak in instances of Inbox where the next
//!      expected packet is never received and we continue to collect later
//!      packets forever. A bug that prevents any packet from being redelivered
//!      would cause this to happen, which is a pretty significant bug overall.
//!
//! This is by no means a complete list! I've included it as a reminder of the
//! work left to make Oxen a solid solution. Much of what ircd-oxide claims to
//! be rests on the abstractions that Oxen provides being airtight. If Oxen
//! cannot handle failure modes gracefully, then the rest of the IRCD cannot
//! hope to.  I've spent a lot of the past week rushing a framework to allow
//! further growth and an implementation that will work well enough to support
//! oxide development elsewhere, but I've cut a lot of corners in the process
//! and am simply noting some of the harder problems that I've glossed over in
//! the name of simplicity so that they're not missed when I come back for round
//! 2.

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

const REACHABILITY_THRESH: i64 = 20;

/// The token type used to identify Oxen timers. When a timer is created, Oxen
/// requests a `Timer` to be used to refer to it when the timer fires or is
/// canceled.
pub type Timer = u64;

/// The main Oxen control structure.
pub struct Oxen {
    me: Sid,
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
    ka_cleanup_timer: Timer,
}

/// An event that the user can handle.
pub enum OxenEvent {
    /// An incoming message. `Sid` identifies the source.
    Message(Sid, Vec<u8>),
    /// A peer has become visible, identified by the `Sid`.
    PeerVisible(Sid),
    /// A peer is no longer visible, identified by the `Sid`.
    PeerVanished(Sid),
}

/// A trait implemented by the protocol user, designed to decouple Oxen from the
/// implementation layer as much as possible.
///
/// An effort has been made to keep this trait small so as not to overwhelm the
/// implementor.
pub trait OxenHandler {
    /// Returns the current time as a `Timespec`. `time::get_time` is sufficient
    /// for most practical applications.
    fn now(&self) -> Timespec;

    /// Queues an XENC value to be sent to the given peer. The message can be
    /// sent immediately, or placed in a queue to be sent later. The specifics
    /// are left to the implementor, but this should reasonably function like
    /// `sendto` on a UDP socket would.
    fn queue_send<X>(&mut self, peer: Sid, data: X)
    where xenc::Value: From<X>;

    /// Sets a timer to fire after the given duration has passed. Ideally, this
    /// will happen exactly after the specified duration has passed, (so the
    /// timer fires at `now` plus `at`), but Oxen does not rely on microsecond
    /// precision for this. (Try to keep it within a few hundred ms though.) The
    /// caller should invoke `timeout` on the associated `Oxen` when the timer
    /// fires.
    fn timer_set(&mut self, at: Duration) -> Timer;

    /// Indicates that Oxen is no longer interested in the given timer. While
    /// it's technically a logic error to invoke `timeout` after a timer has
    /// been canceled, Oxen is pretty tolerant of this. This is merely a
    /// courtesy to indicate to the user that the resources associated with the
    /// timer can be released.
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
    /// Creates a new `Oxen` identified by the given `Sid`.
    pub fn new<H>(hdlr: &mut H, me: Sid) -> Oxen
    where H: OxenHandler {
        let mut oxen = Oxen {
            me: me,
            peers: HashSet::new(),

            lc: LastContact::new(me),
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
            ka_cleanup_timer: 0,
        };

        oxen.peers.insert(me);

        // start these timers
        oxen.last_contact_gossip(hdlr);
        oxen.check_last_contact(hdlr);
        oxen.clean_old_keepalives(hdlr);

        oxen
    }

    /// Dumps statistics about `self` as `info!` messages.
    pub fn dump_stats<H>(&self, hdlr: &mut H)
    where H: OxenHandler {
        info!("stats for {}", self.me);

        info!("  pending keepalives      : {:6}", self.pending_ka.len());
        info!("  pending messages        : {:6}", self.pending_msgs.len());
        info!("  pending message timers  : {:6}", self.pending_msg_timers.len());

        info!("  last contact (since now)");
        let now = hdlr.now();
        let cols = self.peers.iter().fold(String::new(), |s, p| {
            format!("{} {}", s, p)
        });
        info!("       {}", cols);
        for p in self.peers.iter() {
            let row = self.peers.iter().fold(String::new(), |s, q| {
                if *p == *q {
                    format!("{}   -", s)
                } else {
                    let t = (now - self.lc.get(p, q)).num_seconds();
                    if t >= 1000 {
                        format!("{} >??", s)
                    } else if t <= -100 {
                        format!("{} <??", s)
                    } else {
                        format!("{} {:3}", s, t)
                    }
                }
            });
            info!("    {}{}", *p, row);
        }

        info!("  peer statuses");

        info!("  broadcast inboxes");
        self.brd_inbox.dump_stats();
        info!("  one to one inboxes");
        self.one_inbox.dump_stats();
    }

    /// Called to make Oxen aware of the given peer. This only needs to be
    /// called once for every peer in the cluster. For example, if a peer
    /// vanishes, Oxen will still remember that the peer was visible at some
    /// point and not need to be reminded that the peer exists. However,
    /// although from the user's perspective Oxen is relatively stateless in
    /// keeping track of peers, it needs to do some initial setup before the
    /// protocol can be used correctly, which is the purpose of this function.
    pub fn add_peer<H>(&mut self, hdlr: &mut H, sid: Sid)
    where H: OxenHandler {
        if sid == self.me {
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

    /// Called when an XENC value arrives from a peer to be processed by Oxen.
    /// The callback `cb` is called with any events that result from processing
    /// of this message.
    pub fn incoming<H, F>(
        &mut self,
        hdlr: &mut H,
        from: Sid,
        data: xenc::Value,
        cb: F
    ) where H: OxenHandler, F: FnMut(&mut H, OxenEvent) {
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
                Some(at) => self.lc.put(self.me, from, at),
                _ => warn!("stray keepalive {} from {}", kk, from),
            }
        }

        match p.body {
            ParcelBody::Missing => { },
            ParcelBody::MsgData(data) => self.handle_msg_data(hdlr, data, cb),
            ParcelBody::MsgAck(data) => self.handle_msg_ack(hdlr, data),
            ParcelBody::LcGossip(data) => self.handle_lc_gossip(hdlr, data),
        }
    }

    /// Called when a timer fires. The callback `cb` is called with any events
    /// that result from this timer firing.
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

        if timer == self.ka_cleanup_timer {
            self.clean_old_keepalives(hdlr);
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

    /// Sends a blob of octets to all peers Oxen is aware of (i.e., that have
    /// been added with `add_peer`).
    pub fn send_broadcast<H>(&mut self, hdlr: &mut H, data: Vec<u8>)
    where H: OxenHandler {
        let peers: Vec<Sid> = self.peers.iter().cloned().collect();

        let brd_seq = self.brd_seq.wrapping_add(1);
        self.brd_seq = brd_seq;

        for p in peers {
            if p == self.me {
                continue;
            }

            self.send_with_redelivery(hdlr, &p, MsgDataBody::MsgBrd(MsgBrd {
                seq: brd_seq,
                data: data.clone(),
            }));
        }
    }

    /// Sends a blob of octets to a single peer. Oxen must be aware of the peer,
    /// i.e., it must have been added with `add_peer`.
    pub fn send_one<H>(&mut self, hdlr: &mut H, to: Sid, data: Vec<u8>)
    where H: OxenHandler {
        if to == self.me {
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
        if *to == self.me {
            error!("oxen tried to send a message to itself! dropping.");
            return;
        }

        let ival = Duration::milliseconds(800);

        let id = random();

        let msg = MsgData {
            to: *to,
            fr: self.me,
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
        if *to == self.me {
            error!("oxen tried to send a routed message to itself! dropping.");
            return false;
        }

        let thresh = Duration::seconds(REACHABILITY_THRESH);

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
            if *p == self.me {
                continue;
            }

            let lc = self.lc.get(&self.me, p);
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
                .reachable(p, hdlr.now(), Duration::seconds(REACHABILITY_THRESH));

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

    fn clean_old_keepalives<H>(&mut self, hdlr: &mut H)
    where H: OxenHandler {
        self.ka_cleanup_timer = hdlr.timer_set(Duration::seconds(20));

        let expired: Vec<_> = self.pending_ka
            .iter()
            .filter_map(|(k, v)| {
                if (hdlr.now() - *v).num_seconds() > REACHABILITY_THRESH {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        for k in expired.iter() {
            self.pending_ka.remove(k);
        }
    }

    fn handle_msg_data<H, F>(&mut self, hdlr: &mut H, data: MsgData, mut cb: F)
    where H: OxenHandler, F: FnMut(&mut H, OxenEvent) {
        // simply forward the message if not to me

        if data.to != self.me {
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
                    fr: self.me,
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
                let fr = data.fr.clone();
                self.brd_inbox.get_mut(data.fr).incoming(brd.seq, brd.data, |d| {
                    cb(hdlr, OxenEvent::Message(fr, d))
                });
            },

            MsgDataBody::MsgOne(one) => {
                let fr = data.fr.clone();
                self.one_inbox.get_mut(data.fr).incoming(one.seq, one.data, |d| {
                    cb(hdlr, OxenEvent::Message(fr, d))
                });
            },

            _ => { },
        }
    }

    fn handle_msg_ack<H>(&mut self, hdlr: &mut H, data: MsgAck)
    where H: OxenHandler {
        // simply forward the acknowledgement if not to me

        if data.to != self.me {
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
            if *p == self.me {
                continue;
            }

            hdlr.queue_send(*p, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::LcGossip(gossip.clone()),
            });
        }
    }

    fn handle_lc_gossip<H>(&mut self, _hdlr: &mut H, data: LcGossip)
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
    syn_seq: SeqNum,
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
            syn_seq: 0,
            next_seq: 0,
            pending: BinaryHeap::new(),
            _kind: PhantomData
        }
    }

    fn synchronize(&mut self, seq: SeqNum) {
        if !self.synchronized {
            self.synchronized = true;
            self.syn_seq = seq;
            self.next_seq = seq + 1;
        } else if seq != self.syn_seq {
            error!("logic error: already synchronized!");
        }
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

            if let Some(m) = self.pending.pop() {
                if m.seq == self.next_seq {
                    deliver(m.data);
                    self.next_seq = self.next_seq.wrapping_add(1);
                } else if m.seq > self.next_seq {
                    self.pending.push(m);
                    error!("logic error: tried to eat before anything ready");
                    return;
                }
            } else {
                error!("logic error: good peek followed by bad pop?");
                return;
            }
        }
    }

    fn dump_stats(&self, sid: Sid) {
        if self.synchronized {
            info!("    {}: pending: {:3}  next_seq: {:10}",
                    sid, self.pending.len(), self.next_seq);
            for pending in self.pending.iter() {
                info!("      {} {}", pending.seq,
                    String::from_utf8_lossy(&pending.data[..]));
            }
        } else {
            info!("    {}: (not synchronized)", sid);
        }
    }
}

impl<Kind> Inboxes<Kind> {
    fn new() -> Inboxes<Kind> {
        Inboxes {
            map: HashMap::new(),
        }
    }

    fn dump_stats(&self) {
        for (sid, inbox) in self.map.iter() {
            inbox.dump_stats(*sid);
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

#[test]
fn test_inbox_resync() {
    let mut inbox: Inbox<Broadcast> = Inbox::new();
    let mut received = Vec::new();

    inbox.synchronize(99);
    inbox.incoming(103, b"d".to_vec(), |v| received.push(v[0]));
    inbox.incoming(100, b"a".to_vec(), |v| received.push(v[0]));
    inbox.incoming(101, b"b".to_vec(), |v| received.push(v[0]));
    inbox.synchronize(99);
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
