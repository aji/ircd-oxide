// state/clock.rs -- Lamport clocks
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Lamport clocks based on real world time.
//!
//! The general idea of these clocks is that there exists a total ordering on
//! them. In other words, for any two clocks, either they are equal or one
//! supersedes the other. This is in contrast to more involved ordering
//! mechanisms like vector clocks that may sometimes not wholly dominate each
//! other. Given this, we can imagine a single global timeline, with distinct
//! clocks appearing at unique points on the timeline.
//!
//! Note that newly created clocks are *guaranteed* to be unique. However,
//! clocks can be cloned, in which case the clone is considered equal. If a
//! clock compares as equal to another clock, it can be safely concluded that
//! one was cloned from the other and they represent the same event.

use std::cmp;
use std::fmt;
use time;

use state::StateItem;
use util::Sid;

/// A basic clock implementation. Ties on timestamps are resolved by using the
/// `sid` field.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Clock {
    time: time::Timespec,
    sid:  Sid,
}

impl Clock {
    /// Constructs a `Clock` corresponding to the current moment in time.
    pub fn now(sid: Sid) -> Clock {
        Clock {
            time: time::get_time(),
            sid:  sid,
        }
    }

    /// Constructs a `Clock` that is older than every other clock.
    pub fn neg_infty() -> Clock {
        Clock {
            time: time::Timespec { sec: i64::min_value(), nsec: 0 },
            sid:  Sid::identity()
        }
    }

    /// Constructs a `Clock` that is newer than every other clock.
    pub fn pos_infty() -> Clock {
        Clock {
            time: time::Timespec { sec: i64::max_value(), nsec: 0 },
            sid:  Sid::identity()
        }
    }

    #[cfg(test)]
    pub fn at(t: i64) -> Clock {
        Clock {
            time: time::Timespec { sec: t, nsec: 0 },
            sid:  Sid::identity()
        }
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Clock({}.{:03}-{})",
                self.time.sec, self.time.nsec / 1000000, self.sid)
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
    fn merge(&mut self, other: &Clock) -> &mut Clock {
        if *self < *other {
            self.time  = other.time;
            self.sid   = other.sid;
        }

        self
    }
}
