// oxen/lc.rs -- last contact
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use time;

use util::Sid;
use util::Table;

// timestamp representing negative infinity
const NEG_INFTY: f64 = 0.0;

pub struct LastContact {
    me: Sid,
    peers: HashSet<Sid>,
    tab: Table<Sid, f64>,
}

impl LastContact {
    pub fn new(me: Sid) -> LastContact {
        let mut peers = HashSet::new();
        peers.insert(me);
        LastContact { me: me, peers: peers, tab: Table::new() }
    }

    pub fn get(&self, from: &Sid, to: &Sid) -> f64 {
        self.tab.get(from, to).map(|t| *t).unwrap_or(NEG_INFTY)
    }

    pub fn put(&mut self, from: Sid, to: Sid, time: f64) {
        self.peers.insert(from);
        self.peers.insert(to);
        self.tab.put(from, to, time);
    }

    pub fn usable(&self, from: &Sid, to: &Sid, now: f64, thresh: f64) -> bool {
        from == to || self.get(from, to) > now - thresh
    }

    pub fn reachable(&self, to: &Sid, now: f64, thresh: f64) -> bool {
        for p in self.peers.iter() {
            if self.usable(p, to, now, thresh) {
                return true;
            }
        }

        false
    }

    pub fn route(&self, to: &Sid, now: f64, thresh: f64) -> Option<Sid> {
        let mut distances: HashMap<Sid, isize> = HashMap::new();
        let mut parents: HashMap<Sid, Sid> = HashMap::new();

        let mut queue: VecDeque<Sid> = VecDeque::new();

        println!("");

        distances.insert(self.me, 0);
        queue.push_back(self.me);

        loop {
            let u = match queue.pop_front() {
                Some(u) => u,
                None => return None
            };
            println!("dequeue: {}", u);

            let distance = distances.get(&u).cloned().unwrap();

            for n in self.peers.iter() {
                println!("  {} -> {}", u, n);
                if !self.usable(&u, n, now, thresh) {
                    println!("    not usable");
                    continue;
                }

                if n != to {
                    println!("    not target");
                    distances.entry(*n).or_insert_with(|| {
                        parents.insert(*n, u);
                        queue.push_back(*n);
                        println!("      distance = {}", distance + 1);
                        distance + 1
                    });
                    continue;
                }

                println!("    target!");

                parents.insert(*n, u);
                let mut at = n;

                loop {
                    println!("      <-- {}", at);
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

    assert_eq!(Some(n1), lc.route(&n1, now, 10.0));
    assert_eq!(Some(n1), lc.route(&n2, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n3, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n4, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n5, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n6, now, 10.0));
    assert_eq!(Some(n3), lc.route(&n7, now, 10.0));
}
