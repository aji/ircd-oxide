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

use common::Sid;

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

/// A value that has an associated timestamp, and whose merge rules are based on
/// taking the value with the newer clock.
#[derive(Clone)]
pub struct Clocked<T: Clone> {
    clock: Clock,
    data: T
}

impl<T: Clone> Clocked<T> {
    /// Creates a new `Clocked` that will be superseded by all other `Clocked`s.
    pub fn new(data: T) -> Clocked<T> {
        Clocked {
            clock: Clock::neg_infty(),
            data: data
        }
    }

    /// Creates a `Clocked` that is tagged with the current time and given
    /// `Sid`.
    pub fn now(sid: Sid, data: T) -> Clocked<T> {
        Clocked {
            clock: Clock::now(sid),
            data: data
        }
    }
}

impl<T: Clone> ::std::ops::Deref for Clocked<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: Clone> ::std::cmp::PartialEq for Clocked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.clock == other.clock
    }
}

impl<T: Clone> ::std::cmp::Eq for Clocked<T> { }
