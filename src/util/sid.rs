// util/sid.rs -- server ID type, used in several places
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

extern crate time;

use std::fmt;

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
