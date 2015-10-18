// oxen/lc.rs -- last contact
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The "last contact" table.
//!
//! The last contact table is used for making a number of decisions in Oxen,
//! particularly message routing and deciding whether to give up on a peer.
//! When a message is sent for delivery, the time of the first attempt is
//! recorded. When the message is acknowledged, the time of the first attempt is
//! used as the "last contact" time. This is to prevent high latency from making
//! hosts appear more reachable than they actually are.
//!
//! Last contact information is merely a heuristic, and should never be
//! interpreted as indicating anything certain about the network. However, it's
//! useful to have a vague idea of what may or may not fail, and the last
//! contact table provides that.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use time;

use util::Sid;
use util::Table;

// timestamp representing negative infinity
const NEG_INFTY: f64 = 0.0;

/// The last contact table. See the [module level documentation](index.html)
/// for more information.
pub struct LastContact {
    me: Sid,
    peers: HashSet<Sid>,
    tab: Table<Sid, f64>,
}

impl LastContact {
    /// Creates a new `LastContact` instance, with `me` corresponding to the SID
    /// of this node.
    pub fn new(me: Sid) -> LastContact {
        let mut peers = HashSet::new();
        peers.insert(me);
        LastContact { me: me, peers: peers, tab: Table::new() }
    }

    /// Fetches the time of the last contact between two given nodes. If the
    /// requested information is not known, some arbitrary timestamp, well in
    /// the past, is returned.
    pub fn get(&self, from: &Sid, to: &Sid) -> f64 {
        self.tab.get(from, to).map(|t| *t).unwrap_or(NEG_INFTY)
    }

    /// Puts the last contact time in the table.
    pub fn put(&mut self, from: Sid, to: Sid, time: f64) {
        self.peers.insert(from);
        self.peers.insert(to);
        self.tab.put(from, to, time);
    }

    /// Determines if the indicated link is possibly usable, given some current
    /// time and a threshold time delta. If the last contact time is before
    /// `now - thresh`, the link is considered "probably unusable".
    pub fn usable(&self, from: &Sid, to: &Sid, now: f64, thresh: f64) -> bool {
        from == to || self.get(from, to) > now - thresh
    }

    /// Determines if the indicated peer is possibly reachable, given some
    /// current time and a threshold time delta. If there is no link *to* the
    /// peer with a last contact time within the threshold, the peer is
    /// considered unreachable.
    pub fn reachable(&self, to: &Sid, now: f64, thresh: f64) -> bool {
        for p in self.peers.iter() {
            if self.usable(p, to, now, thresh) {
                return true;
            }
        }

        false
    }

    /// Attempts to find the first node along a possibly usable path from this
    /// node (`self.me`) to peer `to`, given some current time and a threshold
    /// time delta. If no usable path can be found (i.e. we appear to be totally
    /// partioned from `to`) then `None` is returned.
    pub fn route(&self, to: &Sid, now: f64, thresh: f64) -> Option<Sid> {
        let mut distances: HashMap<Sid, isize> = HashMap::new();
        let mut parents: HashMap<Sid, Sid> = HashMap::new();

        let mut queue: VecDeque<Sid> = VecDeque::new();

        distances.insert(self.me, 0);
        queue.push_back(self.me);

        loop {
            let u = match queue.pop_front() {
                Some(u) => u,
                None => return None
            };

            let distance = distances.get(&u).cloned().unwrap();

            for n in self.peers.iter() {
                if !self.usable(&u, n, now, thresh) {
                    continue;
                }

                if n != to {
                    distances.entry(*n).or_insert_with(|| {
                        parents.insert(*n, u);
                        queue.push_back(*n);
                        distance + 1
                    });
                    continue;
                }

                parents.insert(*n, u);
                let mut at = n;

                loop {
                    match parents.get(at) {
                        Some(p) if *p == self.me => return Some(*at),
                        Some(p)                  => at = p,
                        None                     => return None,
                    }
                }
            }
        }
    }
}

#[test]
fn test_route_undirected() {
    let me = Sid::new("0ME");
    let n1 = Sid::new("0N1");
    let n2 = Sid::new("0N2");
    let n3 = Sid::new("0N3");
    let n4 = Sid::new("0N4");
    let n5 = Sid::new("0N5");
    let n6 = Sid::new("0N6");
    let n7 = Sid::new("0N7");

    let now: f64 = 100.0;

    //  me--n1--n2--n3
    //   |       |
    //  n4--n5  n6  n7

    let lc = {
        let mut lc = LastContact::new(me);

        lc.put(me, n1, now); lc.put(n1, me, now);
        lc.put(n1, n2, now); lc.put(n2, n1, now);
        lc.put(n2, n3, now); lc.put(n3, n2, now);
        lc.put(n2, n6, now); lc.put(n6, n2, now);
        lc.put(me, n4, now); lc.put(n4, me, now);
        lc.put(n4, n5, now); lc.put(n5, n4, now);

        lc.put(n7, n7, now); // a little contrived

        lc
    };

    assert_eq!(Some(me), lc.route(&me, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n1, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n2, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n3, now, 10.0));
    assert_eq!(Some(n4), lc.route(&n4, now, 10.0));
    assert_eq!(Some(n4), lc.route(&n5, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n6, now, 10.0));
    assert_eq!(None,     lc.route(&n7, now, 10.0));
}

#[test]
fn test_route_directed() {
    let me = Sid::new("0ME");
    let n1 = Sid::new("0N1");
    let n2 = Sid::new("0N2");
    let n3 = Sid::new("0N3");
    let n4 = Sid::new("0N4");
    let n5 = Sid::new("0N5");
    let n6 = Sid::new("0N6");
    let n7 = Sid::new("0N7");

    let now: f64 = 100.0;

    // me <--> n1 <--> n2 <--- n6 <--- n7
    //  ^                               ^
    //  |                               |
    //  +----> n3 <--> n4 <--> n5 <-----+

    let lc = {
        let mut lc = LastContact::new(me);

        lc.put(me, n1, now);
        lc.put(me, n3, now);
        lc.put(n1, me, now);
        lc.put(n1, n2, now);
        lc.put(n2, n1, now);
        lc.put(n3, me, now);
        lc.put(n3, n4, now);
        lc.put(n4, n3, now);
        lc.put(n4, n5, now);
        lc.put(n5, n4, now);
        lc.put(n5, n7, now);
        lc.put(n6, n2, now);
        lc.put(n7, n5, now);
        lc.put(n7, n6, now);

        lc
    };

    assert_eq!(Some(me), lc.route(&me, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n1, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n2, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n3, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n4, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n5, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n6, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n7, now, 10.0));
}
