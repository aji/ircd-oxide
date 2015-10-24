// util/sid.rs -- server ID type, used in several places
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

extern crate time;

use std::convert::From;
use std::fmt;

use xenc;

#[derive(Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Sid([u8; 3]);

impl Sid {
    pub fn new(s: &str) -> Sid {
        let s = s.as_bytes();
        Sid([s[0], s[1], s[2]])
    }

    pub fn identity() -> Sid {
        Sid::new("000")
    }
}

impl<'a> From<&'a [u8]> for Sid {
    fn from(v: &[u8]) -> Sid {
        match v.len() {
            0 => Sid([   0,    0,    0]),
            1 => Sid([v[0],    0,    0]),
            2 => Sid([v[0], v[1],    0]),
            _ => Sid([v[0], v[1], v[2]]),
        }
    }
}

impl From<Sid> for Vec<u8> {
    fn from(s: Sid) -> Vec<u8> {
        vec![s.0[0], s.0[1], s.0[2]]
    }
}

impl fmt::Debug for Sid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0[..]))
    }
}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0[..]))
    }
}

// SIDs might some day get special coding, but for now we just encode as Vec<u8>

impl From<Sid> for xenc::Value {
    fn from(sid: Sid) -> xenc::Value {
        xenc::Value::Octets(From::from(sid))
    }
}

impl xenc::FromXenc for Sid {
    fn from_xenc(v: xenc::Value) -> xenc::Result<Sid> {
        v.into_octets().map(|s| Sid::from(&s[..])).ok_or(xenc::Error)
    }
}
