// oxen/core.rs -- the Oxen core
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The core Oxen logic

use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::From;
use std::marker::PhantomData;

use oxen::OxenBack;
use oxen::lc::LastContact;
use util::Sid;
use xenc;
use xenc::FromXenc;

pub struct Oxen<B: OxenBack> {
    me: Sid,
    peers: HashSet<Sid>,
    _back: PhantomData<B>
}

impl<B: OxenBack> Oxen<B> {
    pub fn new(me: Sid) -> Oxen<B> {
        Oxen {
            me: me,
            peers: {
                let mut h = HashSet::new();
                h.insert(me);
                h
            },
            _back: PhantomData
        }
    }

    pub fn add_peer(&mut self, back: &mut B, sid: Sid) {
        self.peers.insert(sid);
    }

    pub fn forget_peer(&mut self, back: &mut B, sid: Sid) {
        self.peers.remove(&sid);
    }

    pub fn incoming(&mut self, back: &mut B, from: Option<Sid>, data: Vec<u8>) {
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

        if pkt.to != self.me {
            self.forward(back, pkt);
        } else {
            self.handle(back, pkt);
        }
    }

    pub fn timeout(&mut self, back: &mut B, timer: B::Timer) {
    }

    pub fn send_broadcast(&mut self, back: &mut B, data: Vec<u8>) {
    }

    pub fn send_one(&mut self, back: &mut B, to: Sid, data: Vec<u8>) {
    }

    fn forward(&mut self, back: &mut B, pkt: Packet) {
    }

    fn handle(&mut self, back: &mut B, pkt: Packet) {
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
