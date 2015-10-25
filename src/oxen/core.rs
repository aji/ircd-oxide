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
use time::{Duration, Timespec};

use oxen::OxenBack;
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
    pub fn new<B>(back: &mut B) -> Oxen
    where B: OxenBack {
        let mut oxen = Oxen {
            peers: HashSet::new(),

            lc: LastContact::new(back.me()),
            peer_status: HashMap::new(),

            pending_ka: HashMap::new(),
            pending_msgs: HashMap::new(),
            pending_msg_timers: HashMap::new(),

            lc_timer: 0,
        };

        oxen.peers.insert(back.me());

        // start these timers
        oxen.check_last_contact(back);

        oxen
    }

    pub fn add_peer<B>(&mut self, back: &mut B, sid: Sid)
    where B: OxenBack {
        self.peers.insert(sid);
        self.peer_status.insert(sid, PeerStatus::Unchecked);

        info!("synchronizing with {}", sid);
        self.send_with_redelivery(back, &sid, MsgDataBody::MsgSync(MsgSync {
            brd: 0,
            one: 0,
        }));
    }

    pub fn incoming<B>(&mut self, back: &mut B, from: Sid, data: Vec<u8>)
    where B: OxenBack {
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
            back.queue_send_xenc(from, Parcel {
                ka_rq: None,
                ka_ok: Some(ka),
                body: ParcelBody::Missing,
            });
        }

        if let Some(kk) = p.ka_ok {
            debug!("received keepalive {} ok from {}", kk, from);

            match self.pending_ka.remove(&from) {
                Some(pka) if pka.id == kk => {
                    self.lc.put(back.me(), from, pka.at);
                },
                Some(pka) => { // pka.id != kk
                    self.pending_ka.insert(from, pka);
                },
                _ => info!("stray keepalive {} from {}", kk, from),
            }
        }

        match p.body {
            ParcelBody::Missing => { },
            ParcelBody::MsgData(data) => self.handle_msg_data(back, data),
            ParcelBody::MsgAck(data) => self.handle_msg_ack(back, data),
            ParcelBody::LcGossip(data) => self.handle_lc_gossip(back, data),
        }
    }

    pub fn timeout<B>(&mut self, back: &mut B, timer: Timer)
    where B: OxenBack {
        if timer == self.lc_timer {
            self.check_last_contact(back);
            return;
        }

        match self.pending_msg_timers.remove(&timer) {
            Some(k) => match self.pending_msgs.remove(&k) {
                Some(pending) => self.redeliver(back, pending),
                _ => error!("inconsistency in pending message tables!"),
            },
            _ => error!("unknown timer has fired!"),
        };
    }

    pub fn send_broadcast<B>(&mut self, back: &mut B, data: Vec<u8>)
    where B: OxenBack {
        let peers: Vec<Sid> = self.peers.iter().cloned().collect();

        for p in peers {
            self.send_with_redelivery(back, &p, MsgDataBody::MsgBrd(MsgBrd {
                seq: 0,
                data: data.clone(),
            }));
        }
    }

    pub fn send_one<B>(&mut self, back: &mut B, to: Sid, data: Vec<u8>)
    where B: OxenBack {
        self.send_with_redelivery(back, &to, MsgDataBody::MsgOne(MsgOne {
            seq: 0,
            data: data,
        }));
    }

    fn redeliver<B>(&mut self, back: &mut B, mut pending: PendingMessage)
    where B: OxenBack {
        let key = (pending.to, pending.id);

        // TODO: fail after a number of retries
        // TODO: retry in longer intervals

        pending.redeliver = back.timer_set(pending.interval);

        // TODO: check this return value
        self.routed(back, &pending.to, Parcel {
            ka_rq: None,
            ka_ok: None,
            body: ParcelBody::MsgData(pending.msg.clone()),
        });

        self.pending_msg_timers.insert(pending.redeliver, key);
        self.pending_msgs.insert(key, pending);
    }

    fn send_with_redelivery<B>(&mut self, back: &mut B, to: &Sid, m: MsgDataBody)
    where B: OxenBack {
        let ival = Duration::milliseconds(800);

        let id = random();

        let msg = MsgData {
            to: *to,
            fr: back.me(),
            id: Some(id),
            body: m,
        };

        let pending = PendingMessage {
            to: *to,
            id: id,
            redeliver: back.timer_set(ival),
            interval: ival,
            msg: msg.clone(),
        };

        // TODO: check this return value
        self.routed(back, to, Parcel {
            ka_rq: None,
            ka_ok: None,
            body: ParcelBody::MsgData(msg),
        });

        self.pending_msg_timers.insert(pending.redeliver, (*to, id));
        self.pending_msgs.insert((*to, id), pending);
    }

    fn routed<B, X>(&mut self, back: &mut B, to: &Sid, data: X) -> bool
    where B: OxenBack, xenc::Value: From<X> {
        let thresh = Duration::seconds(20);

        let route = match self.lc.route(to, back.get_time(), thresh) {
            Some(r) => r,
            None => return false,
        };

        back.queue_send_xenc(route, data);

        true
    }

    fn check_last_contact<B>(&mut self, back: &mut B)
    where B: OxenBack {
        self.lc_timer = back.timer_set(Duration::milliseconds(1000));

        for p in self.peers.iter() {
            if back.me() == *p {
                continue;
            }

            let lc = self.lc.get(&back.me(), p);
            let age = (back.get_time() - lc).num_seconds();

            if age >= 2 {
                debug!("sending keepalive to {}", p);
                let ka = random();
                self.pending_ka.insert(*p, PendingKeepalive {
                    id: ka,
                    at: back.get_time(),
                });
                back.queue_send_xenc(*p, Parcel {
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

    fn handle_msg_data<B>(&mut self, back: &mut B, data: MsgData)
    where B: OxenBack {
        if data.to != back.me() {
            back.queue_send_xenc(data.to, Parcel {
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
                    fr: back.me(),
                    id: id,
                })
            };
            back.queue_send_xenc(data.to, parcel);
        }

        if let MsgDataBody::MsgSync(syn) = data.body {
            info!("got synchronization from {}", data.fr);
        }
    }

    fn handle_msg_ack<B>(&mut self, back: &mut B, data: MsgAck)
    where B: OxenBack {
        if data.to != back.me() {
            back.queue_send_xenc(data.to, Parcel {
                ka_rq: None,
                ka_ok: None,
                body: ParcelBody::MsgAck(data),
            });
            return;
        }

        if let Some(pending) = self.pending_msgs.remove(&(data.fr, data.id)) {
            back.timer_cancel(pending.redeliver);
            self.pending_msg_timers.remove(&pending.redeliver);
        }
    }

    fn handle_lc_gossip<B>(&mut self, back: &mut B, data: LcGossip)
    where B: OxenBack {
    }
}
