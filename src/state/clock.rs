// state/clock.rs -- Lamport clocks
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! Lamport clocks

use std::cmp;
use time;

use state::StateItem;

pub type Sid = u64;

pub const IDENTITY_SID: Sid = 0;

/// A basic Lamport clock implementation. Ties on timestamps are resolved by
/// using the `sid` field.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Clock {
    time: time::Timespec,
    sid:  Sid,
}

impl Clock {
    /// Constructs a `Clock` corresponding to the current moment in time.
    pub fn now(sid: Sid) -> Clock {
        assert!(sid != IDENTITY_SID);

        Clock {
            time: time::get_time(),
            sid:  sid,
        }
    }
}

impl cmp::PartialOrd for Clock {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for Clock {
    // although deriving `Ord` on `Clock` will get us a lexicographic ordering
    // as well, I'd rather do it explicitly, seeing how critical the function
    // of this operator is to the integrity of the network's shared state.

    fn cmp(&self, other: &Self) -> cmp::Ordering {
        use std::cmp::Ordering::*;

        match self.time.cmp(&other.time) {
            Less => Less,
            Greater => Greater,
            Equal => self.sid.cmp(&other.sid),
        }
    }
}

impl StateItem for Clock {
    fn identity() -> Clock {
        Clock {
            time: time::Timespec { sec: 0, nsec: 0 },
            sid:  IDENTITY_SID,
        }
    }

    fn merge(&mut self, other: &Clock) -> &mut Clock {
        if *self < *other {
            self.time  = other.time;
            self.sid   = other.sid;
        }

        self
    }
}
