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

    unack_timer: Timer,
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

impl Oxen {
    pub fn new<B>(back: &mut B) -> Oxen
    where B: OxenBack {
        let mut oxen = Oxen {
            peers: HashSet::new(),

            lc: LastContact::new(back.me()),
            peer_status: HashMap::new(),

            pending_ka: HashMap::new(),

            unack_timer: 0,
            lc_timer: 0,
        };

        oxen.peers.insert(back.me());

        // start these timers
        oxen.check_unacked_packets(back);
        oxen.check_last_contact(back);

        oxen
    }

    pub fn add_peer<B>(&mut self, back: &mut B, sid: Sid)
    where B: OxenBack {
        self.peers.insert(sid);
        self.peer_status.insert(sid, PeerStatus::Unchecked);
    }

    pub fn forget_peer<B>(&mut self, back: &mut B, sid: Sid)
    where B: OxenBack {
        self.peers.remove(&sid);
        self.peer_status.remove(&sid);
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
    }

    pub fn timeout<B>(&mut self, back: &mut B, timer: Timer)
    where B: OxenBack {
        match timer {
            t if t == self.unack_timer     => self.check_unacked_packets(back),
            t if t == self.lc_timer        => self.check_last_contact(back),

            _ => warn!("unknown timer has fired!"),
        }
    }

    pub fn send_broadcast<B>(&mut self, back: &mut B, data: Vec<u8>)
    where B: OxenBack {
    }

    pub fn send_one<B>(&mut self, back: &mut B, to: Sid, data: Vec<u8>)
    where B: OxenBack {
    }

    fn check_unacked_packets<B>(&mut self, back: &mut B)
    where B: OxenBack {
        self.unack_timer = back.timer_set(Duration::milliseconds(200));
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
}
