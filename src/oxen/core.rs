// oxen/core.rs -- the Oxen core
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The core Oxen logic

#![allow(unused_variables)] // grumble grumble

use rand::random;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::From;
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

    pending_ka: HashMap<Sid, PendingKeepalive>,
    pending_msgs: HashMap<(Sid, MsgId), PendingMessage>,
    pending_msg_timers: HashMap<Timer, (Sid, MsgId)>,

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

    fn queue_send(&mut self, peer: Sid, data: Vec<u8>);

    fn timer_set(&mut self, at: Duration) -> Timer;

    fn timer_cancel(&mut self, timer: Timer);

    fn queue_send_xenc<X>(&mut self, peer: Sid, data: X)
    where xenc::Value: From<X> {
        let mut vec = Vec::new();
        let _ = xenc::Value::from(data).write(&mut vec);
        self.queue_send(peer, vec);
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PeerStatus {
    Unchecked,
    Available,
    Unavailable,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct PendingKeepalive {
    id: KeepaliveId,
    at: Timespec,
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

            lc_timer: 0,
        };

        oxen.peers.insert(hdlr.me());

        // start these timers
        oxen.check_last_contact(hdlr);

        oxen
    }

    pub fn add_peer<H>(&mut self, hdlr: &mut H, sid: Sid)
    where H: OxenHandler {
        self.peers.insert(sid);
        self.peer_status.insert(sid, PeerStatus::Unchecked);

        info!("synchronizing with {}", sid);
        self.send_with_redelivery(hdlr, &sid, MsgDataBody::MsgSync(MsgSync {
            brd: 0,
            one: 0,
        }));
    }

    pub fn incoming<H>(&mut self, hdlr: &mut H, from: Sid, data: Vec<u8>)
    where H: OxenHandler {
        let p = match xenc::Parser::new(&data[..]).next() {
            Ok(p_xenc) => match Parcel::from_xenc(p_xenc) {
                Ok(p) => p,
                Err(_) => {
                    error!("could not decode a Parcel from incoming XENC");
                    return;
                },
            },
            Err(_) => {
                warn!("could not decode XENC from incoming data");
                return;
            },
        };

        if let Some(ka) = p.ka_rq {
            debug!("responding to {} keepalive {}", from, ka);
            hdlr.queue_send_xenc(from, Parcel {
                ka_rq: None,
                ka_ok: Some(ka),
                body: ParcelBody::Missing,
            });
        }

        if let Some(kk) = p.ka_ok {
            debug!("received keepalive {} ok from {}", kk, from);

            match self.pending_ka.remove(&from) {
                Some(pka) if pka.id == kk => {
                    self.lc.put(hdlr.me(), from, pka.at);
                },
                Some(pka) => { // pka.id != kk
                    self.pending_ka.insert(from, pka);
                },
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

        for p in peers {
            self.send_with_redelivery(hdlr, &p, MsgDataBody::MsgBrd(MsgBrd {
                seq: 0,
                data: data.clone(),
            }));
        }
    }

    pub fn send_one<H>(&mut self, hdlr: &mut H, to: Sid, data: Vec<u8>)
    where H: OxenHandler {
        self.send_with_redelivery(hdlr, &to, MsgDataBody::MsgOne(MsgOne {
            seq: 0,
            data: data,
        }));
    }

    fn redeliver<H>(&mut self, hdlr: &mut H, mut pending: PendingMessage)
    where H: OxenHandler {
        let key = (pending.to, pending.id);

        // TODO: fail after a number of retries
        // TODO: retry in longer intervals

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
        let thresh = Duration::seconds(20);

        let route = match self.lc.route(to, hdlr.now(), thresh) {
            Some(r) => r,
            None => return false,
        };

        hdlr.queue_send_xenc(route, data);

        true
    }

    fn check_last_contact<H>(&mut self, hdlr: &mut H)
    where H: OxenHandler {
        self.lc_timer = hdlr.timer_set(Duration::milliseconds(1000));

        for p in self.peers.iter() {
            if hdlr.me() == *p {
                continue;
            }

            let lc = self.lc.get(&hdlr.me(), p);
            let age = (hdlr.now() - lc).num_seconds();

            if age >= 2 {
                debug!("sending keepalive to {}", p);
                let ka = random();
                self.pending_ka.insert(*p, PendingKeepalive {
                    id: ka,
                    at: hdlr.now(),
                });
                hdlr.queue_send_xenc(*p, Parcel {
                    ka_rq: Some(ka),
                    ka_ok: None,
                    body: ParcelBody::Missing,
                });
            }

            let status = self.peer_status
                .entry(*p)
                .or_insert(PeerStatus::Unchecked);

            match *status {
                PeerStatus::Unchecked => {
                    if age < 20 {
                        info!("promoting {} out of unchecked", p);
                        *status = PeerStatus::Available;
                    }
                },

                PeerStatus::Available => {
                    if age >= 20 {
                        info!("{} has become unavailable", p);
                        *status = PeerStatus::Unavailable;
                    }
                },

                PeerStatus::Unavailable => {
                    if age < 20 {
                        info!("{} is available again", p);
                        *status = PeerStatus::Available;
                    }
                },
            }
        }
    }

    fn handle_msg_data<H>(&mut self, hdlr: &mut H, data: MsgData)
    where H: OxenHandler {
        if data.to != hdlr.me() {
            hdlr.queue_send_xenc(data.to, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgData(data),
            });
            return;
        }

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
            hdlr.queue_send_xenc(data.to, parcel);
        }

        if let MsgDataBody::MsgSync(syn) = data.body {
            info!("got synchronization from {}", data.fr);
        }
    }

    fn handle_msg_ack<H>(&mut self, hdlr: &mut H, data: MsgAck)
    where H: OxenHandler {
        if data.to != hdlr.me() {
            hdlr.queue_send_xenc(data.to, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgAck(data),
            });
            return;
        }

        if let Some(pending) = self.pending_msgs.remove(&(data.fr, data.id)) {
            hdlr.timer_cancel(pending.redeliver);
            self.pending_msg_timers.remove(&pending.redeliver);
        }
    }

    fn handle_lc_gossip<H>(&mut self, hdlr: &mut H, data: LcGossip)
    where H: OxenHandler {
    }
}
