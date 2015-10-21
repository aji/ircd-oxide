// oxen/core.rs -- the Oxen core
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The core Oxen logic

#![allow(unused_variables)] // grumble grumble

use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::From;

use oxen::OxenBack;
use oxen::lc::LastContact;
use util::Sid;
use xenc;
use xenc::FromXenc;

pub struct Oxen {
    me: Sid,
    peers: HashSet<Sid>,
}

impl Oxen {
    pub fn new(me: Sid) -> Oxen {
        Oxen {
            me: me,
            peers: {
                let mut h = HashSet::new();
                h.insert(me);
                h
            },
        }
    }

    pub fn add_peer<B>(&mut self, back: &mut B, sid: Sid)
    where B: OxenBack {
        self.peers.insert(sid);
    }

    pub fn forget_peer<B>(&mut self, back: &mut B, sid: Sid)
    where B: OxenBack {
        self.peers.remove(&sid);
    }

    pub fn incoming<B>(&mut self, back: &mut B, from: Option<Sid>, data: Vec<u8>)
    where B: OxenBack {
        let pkt = match xenc::Parser::new(&data[..]).next() {
            Ok(x) => match Packet::from_xenc(x) {
                Some(pkt) => pkt,
                None => {
                    warn!("ignoring bad packet (wrong schema)");
                    return;
                },
            },
            Err(_) => {
                warn!("ignoring bad packet (invalid XENC)");
                return;
            },
        };

        let now = back.get_time();

        if pkt.to != self.me {
            info!("{}.{:03}: {}: forwarding to {}",
                    now.sec, now.nsec / 1000000, self.me, pkt.to);
            back.queue_send(pkt.to, data);
        } else {
            if let Some(sid) = from {
                info!("{}.{:03}: {}: pinging {}",
                        now.sec, now.nsec / 1000000, self.me, sid);
                back.queue_send_xenc(sid, Packet { to: sid });
            }
        }
    }

    pub fn timeout<B>(&mut self, back: &mut B, timer: B::Timer)
    where B: OxenBack {
    }

    pub fn send_broadcast<B>(&mut self, back: &mut B, data: Vec<u8>)
    where B: OxenBack {
    }

    pub fn send_one<B>(&mut self, back: &mut B, to: Sid, data: Vec<u8>)
    where B: OxenBack {
    }

    fn handle<B>(&mut self, back: &mut B, pkt: Packet)
    where B: OxenBack {
    }
}

pub struct Packet {
    to: Sid,
}

impl FromXenc for Packet {
    fn from_xenc(x: xenc::Value) -> Option<Packet> {
        let dict = match x.as_dict() {
            Some(dict) => dict,
            None => return None,
        };

        match dict.get(b"to" as &[u8]) {
            None => None,
            Some(v) => match v.as_octets() {
                None => None,
                Some(s) => Some(Packet {
                    to: From::from(s)
                }),
            },
        }
    }
}

impl From<Packet> for xenc::Value {
    fn from(pkt: Packet) -> xenc::Value {
        xenc::Value::Dict({
            let mut h = HashMap::new();
            h.insert(
                b"to".to_vec(),
                xenc::Value::Octets(From::from(pkt.to))
            );
            h
        })
    }
}
