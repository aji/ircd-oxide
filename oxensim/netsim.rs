use rand::{thread_rng, Rng};
use rand::distributions::{Normal, IndependentSample};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp;
use time::get_time;
use time::{Duration, Timespec};

use ircd::util::{Sid, Table};
use ircd::oxen::{Oxen, OxenBack};

pub type Timer = u64;

struct PendingPacket {
    deliver: Timespec,
    from: Sid,
    to: Sid,
    data: Vec<u8>,
}

impl PartialOrd for PendingPacket {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.deliver.partial_cmp(&other.deliver).map(|o| o.reverse())
    }
}

impl Ord for PendingPacket {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.deliver.cmp(&other.deliver).reverse()
    }
}

impl PartialEq for PendingPacket {
    fn eq(&self, other: &Self) -> bool {
        self.deliver == other.deliver
    }
}

impl Eq for PendingPacket { }

struct PendingTimer {
    fire: Timespec,
    on: Sid,
    token: Timer,
}

impl PartialOrd for PendingTimer {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.fire.partial_cmp(&other.fire).map(|o| o.reverse())
    }
}

impl Ord for PendingTimer {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.fire.cmp(&other.fire).reverse()
    }
}

impl PartialEq for PendingTimer {
    fn eq(&self, other: &Self) -> bool {
        self.fire == other.fire
    }
}

impl Eq for PendingTimer { }

pub enum Event {
    Packet(PendingPacket),
    Timer(PendingTimer),
}

pub struct NetConfig {
    peers: HashSet<Sid>,

    // packet loss, in range 0 to 1, as a ratio of lost packets
    packet_loss: Table<Sid, f64>,

    // distribution for latency figures, expressed in seconds
    latency: Table<Sid, Normal>,
    default_latency: Normal,
}

impl NetConfig {
    pub fn complete(
        peers: &[Sid],
        loss: f64,
        latency_mean: f64, latency_dev: f64
    ) -> NetConfig {
        let mut cfg = NetConfig {
            peers: peers.iter().cloned().collect(),

            packet_loss: Table::new(),

            latency: Table::new(),
            default_latency: Normal::new(latency_mean, latency_dev),
        };

        for p in peers {
            for q in peers {
                cfg.packet_loss.put(*p, *q, loss);
            }
        }

        cfg
    }

    pub fn set_packet_loss(&mut self, from: Sid, to: Sid, loss: f64) {
        self.packet_loss.put(from, to, loss);
    }

    pub fn set_latency(&mut self, from: Sid, to: Sid, mean: f64, dev: f64) {
        self.latency.put(from, to, Normal::new(mean, dev));
    }

    pub fn partition(&mut self, sids: &[Sid]) {
        let qs: HashSet<Sid> = sids.iter().cloned().collect();

        let loss = &mut self.packet_loss;

        for p in self.peers.iter() {
            // skip peers in the given half of the partition
            if qs.contains(&p) {
                continue;
            }

            // otherwise, set loss to 100% in both directions
            for q in qs.iter() {
                loss.put(*p, *q, 1.0);
                loss.put(*q, *p, 1.0);
            }
        }
    }

    fn will_drop_packet(&self, from: &Sid, to: &Sid) -> bool {
        if from == to {
            // realistic networks don't drop packets hosts send to themselves
            return false;
        }

        match self.packet_loss.get(from, to) {
            // the default behavior if no packet loss is configured between
            // nodes is to assume packet loss is 100%!
            None => true,

            // otherwise, roll the dice!
            Some(loss) => thread_rng().next_f64() < *loss,
        }
    }

    fn some_latency_ms(&self, from: &Sid, to: &Sid) -> i64 {
        if from == to {
            // realistic networks don't have latency on the order of
            // milliseconds (usually) for packets hosts send to themselves.
            // but let's add 1ms of latency, just to be safe.
            return 1;
        }

        let dist = match self.latency.get(from, to) {
            None => &self.default_latency,
            Some(dist) => dist,
        };

        cmp::max(0, (dist.ind_sample(&mut thread_rng()) * 1000.0) as i64)
    }
}

pub struct NetSim<'cfg> {
    packets: BinaryHeap<PendingPacket>,
    timers: BinaryHeap<PendingTimer>,
    canceled_timers: HashSet<Timer>,

    config: &'cfg NetConfig,
}

impl<'cfg> NetSim<'cfg> {
    pub fn new(config: &'cfg NetConfig) -> NetSim<'cfg> {
        NetSim {
            packets: BinaryHeap::new(),
            timers: BinaryHeap::new(),
            canceled_timers: HashSet::new(),

            config: config,
        }
    }

    pub fn queue_send(
        &mut self, now: Timespec,
        from: Sid, to: Sid, data: Vec<u8>
    ) {
        // first, decide if we're going to drop the packet
        if self.config.will_drop_packet(&from, &to) {
            return;
        }

        // then, add some latency!
        let latency = {
            let latency_ms = self.config.some_latency_ms(&from, &to);
            Duration::milliseconds(latency_ms)
        };

        // now package it all up and add it to the queue
        self.packets.push(PendingPacket {
            deliver: now + latency,
            from: from,
            to: to,
            data: data
        })
    }

    pub fn queue_timer(&mut self, fire: Timespec, on: Sid, tok: Timer) {
        self.timers.push(PendingTimer {
            fire: fire,
            on: on,
            token: tok
        })
    }

    pub fn cancel_timer(&mut self, tok: Timer) {
        self.canceled_timers.insert(tok);
    }

    fn clear_canceled_timers(&mut self) {
        loop {
            if let Some(tok) = self.timers.peek().map(|t| t.token) {
                if self.canceled_timers.contains(&tok) {
                    self.canceled_timers.remove(&tok);
                    self.timers.pop();
                } else {
                    return;
                }
            } else {
                return;
            }
        }
    }

    pub fn next_event(&mut self) -> Option<Event> {
        self.clear_canceled_timers();

        let take_timer = {
            let next_packet_time = self.packets.peek().map(|p| p.deliver);
            let next_timer_time = self.timers.peek().map(|t| t.fire);

            match next_packet_time {
                Some(pt) => match next_timer_time {
                    Some(tt) => tt < pt,
                    None => false,
                },
                None => true
            }
        };

        if take_timer {
            self.timers.pop().map(|t| Event::Timer(t))
        } else {
            self.packets.pop().map(|p| Event::Packet(p))
        }
    }
}

pub struct BackSim<'r, 'ns: 'r> {
    sim: &'r mut NetSim<'ns>,
    now: Timespec,
    me: Sid,
}

impl<'r, 'ns> OxenBack for BackSim<'r, 'ns> {
    type Timer = Timer;

    fn get_time(&self) -> Timespec { self.now }

    fn queue_send(&mut self, peer: Sid, data: Vec<u8>) {
        self.sim.queue_send(self.now, self.me, peer, data);
    }

    fn timer_set(&mut self, at: Duration) -> Timer {
        let tok = thread_rng().next_u64();
        self.sim.queue_timer(self.now + at, self.me, tok);
        tok
    }

    fn timer_cancel(&mut self, timer: Timer) {
        self.sim.cancel_timer(timer);
    }
}

pub fn run<'cfg>(
    cfg: &'cfg NetConfig,
    mut nodes: HashMap<Sid, Oxen>,
    dur: Duration
) {
    let start = get_time();
    let end = start + dur;

    let mut sim = NetSim::new(cfg);

    loop {
        let evt = match sim.next_event() {
            Some(evt) => evt,
            None => {
                info!("ran out of events");
                return;
            },
        };

        match evt {
            Event::Packet(p) => {
                let mut back = BackSim {
                    sim: &mut sim,
                    now: p.deliver,
                    me: p.to
                };
                if let Some(n) = nodes.get_mut(&p.to) {
                    n.incoming(&mut back, Some(p.from), p.data);
                }
            },

            Event::Timer(t) => {
                let mut back = BackSim {
                    sim: &mut sim,
                    now: t.fire,
                    me: t.on
                };
                if let Some(n) = nodes.get_mut(&t.on) {
                    n.timeout(&mut back, t.token);
                }
            },
        }
    }
}
