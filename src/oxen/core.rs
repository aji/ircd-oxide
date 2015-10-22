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

use oxen::OxenBack;
use oxen::lc::LastContact;
use util::Sid;
use xenc;
use xenc::FromXenc;

pub type Timer = u64;

pub struct Oxen {
    me: Sid,
    peers: HashSet<Sid>,
}

impl Oxen {
    pub fn new<B>(back: &mut B) -> Oxen
    where B: OxenBack {
        Oxen {
            me: back.me(),
            peers: {
                let mut h = HashSet::new();
                h.insert(back.me());
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
    }

    pub fn timeout<B>(&mut self, back: &mut B, timer: Timer)
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
    id: i64,
    from: Sid,
    to: Sid,
    body: PacketBody
}

pub enum PacketBody { }

impl FromXenc for Packet {
    fn from_xenc(x: xenc::Value) -> xenc::Result<Packet> {
        Err(xenc::Error)
    }
}

impl From<Packet> for xenc::Value {
    fn from(pkt: Packet) -> xenc::Value {
        xenc::Value::I64(0)
    }
}
