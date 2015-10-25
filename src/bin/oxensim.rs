#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_must_use)]
#![allow(unused_variables)]
#![allow(unused_mut)]

extern crate rand;
extern crate time;

#[macro_use]
extern crate log;

extern crate ircd;

use rand::{thread_rng, Rng};
use rand::distributions::{Normal, IndependentSample};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp;
use std::str::from_utf8_unchecked;
use std::sync::{Arc, Mutex};
use time::{Duration, Timespec, get_time};

use ircd::util::{Sid, Table};
use ircd::oxen::{Oxen, OxenBack, Timer};

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

enum Event {
    Packet(PendingPacket),
    Timer(PendingTimer),
}

struct NetConfig {
    peers: HashSet<Sid>,

    // packet loss, in range 0 to 1, as a ratio of lost packets
    packet_loss: Table<Sid, f64>,

    // distribution for latency figures, expressed in seconds
    latency: Table<Sid, Normal>,
    default_latency: Normal,
}

impl NetConfig {
    fn complete(
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

    fn set_packet_loss(&mut self, from: Sid, to: Sid, loss: f64) {
        self.packet_loss.put(from, to, loss);
    }

    fn set_latency(&mut self, from: Sid, to: Sid, mean: f64, dev: f64) {
        self.latency.put(from, to, Normal::new(mean, dev));
    }

    fn partition(&mut self, sids: &[Sid]) {
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

struct NetSim<'cfg> {
    log_prefix: Arc<Mutex<String>>,

    packets: BinaryHeap<PendingPacket>,
    timers: BinaryHeap<PendingTimer>,
    canceled_timers: HashSet<Timer>,

    config: &'cfg NetConfig,
}

impl<'cfg> NetSim<'cfg> {
    fn new(config: &'cfg NetConfig, pfx: Arc<Mutex<String>>) -> NetSim<'cfg> {
        NetSim {
            log_prefix: pfx,

            packets: BinaryHeap::new(),
            timers: BinaryHeap::new(),
            canceled_timers: HashSet::new(),

            config: config,
        }
    }

    fn queue_send(
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

    fn queue_timer(&mut self, fire: Timespec, on: Sid, tok: Timer) {
        self.timers.push(PendingTimer {
            fire: fire,
            on: on,
            token: tok
        })
    }

    fn cancel_timer(&mut self, tok: Timer) {
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

    fn next_event(&mut self) -> Option<Event> {
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

struct BackSim<'r, 'ns: 'r> {
    sim: &'r mut NetSim<'ns>,
    now: Timespec,
    me: Sid,
}

impl<'r, 'ns> OxenBack for BackSim<'r, 'ns> {
    fn get_time(&self) -> Timespec { self.now }

    fn me(&self) -> Sid { self.me }

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

fn oxen<'a, 'cfg, S: IndependentSample<f64>>(
    sim: &'a mut NetSim<'cfg>,
    peer: Sid,
    now: Timespec,
    delay: &S
) -> Oxen {
    let del = delay.ind_sample(&mut thread_rng()).abs();

    let mut back = BackSim {
        sim: sim,
        now: now + Duration::milliseconds(del as i64),
        me: peer,
    };

    Oxen::new(&mut back)
}

fn run<'cfg>(
    mut sim: NetSim<'cfg>,
    mut nodes: HashMap<Sid, Oxen>,
    mut now: Timespec,
    dur: Duration
) -> Timespec {
    let end = now + dur;
    let peers: Vec<Sid> = nodes.keys().cloned().collect();

    {
        let delay = Normal::new(0.0, 1000.0);
        for p in peers.iter() {
            for (k, q) in nodes.iter_mut() {
                let del = delay.ind_sample(&mut thread_rng()).abs();
                let mut back = BackSim {
                    sim: &mut sim,
                    now: now + Duration::milliseconds(del as i64),
                    me: *k,
                };
                q.add_peer(&mut back, *p);
            }
        }
    }

    loop {
        let evt = match sim.next_event() {
            Some(evt) => evt,
            None => {
                info!("ran out of events");
                return now;
            },
        };

        now = match evt {
            Event::Packet(p) => {
                { sim.log_prefix.lock().map(|mut s| *s =
                        format!("{}.{:03} {}: ", p.deliver.sec,
                            p.deliver.nsec / 1000000, p.to)); }
                let mut back = BackSim {
                    sim: &mut sim,
                    now: p.deliver,
                    me: p.to
                };
                if let Some(n) = nodes.get_mut(&p.to) {
                    n.incoming(&mut back, p.from, p.data);
                }
                p.deliver
            },

            Event::Timer(t) => {
                { sim.log_prefix.lock().map(|mut s| *s =
                        format!("{}.{:03} {}: ", t.fire.sec,
                            t.fire.nsec / 1000000, t.on)); }
                let mut back = BackSim {
                    sim: &mut sim,
                    now: t.fire,
                    me: t.on
                };
                if let Some(n) = nodes.get_mut(&t.on) {
                    n.timeout(&mut back, t.token);
                }
                t.fire
            },
        };

        { sim.log_prefix.lock().map(|mut s| *s = format!("")); }

        if now > end {
            info!("all done!");
            return now;
        }
    }
}

mod logger {
    use log;
    use std::sync::{Arc, Mutex};

    struct SimpleLogger(Arc<Mutex<String>>);

    impl log::Log for SimpleLogger {
        fn enabled(&self, metadata: &log::LogMetadata) -> bool {
            metadata.level() <= log::LogLevel::Info
        }

        fn log(&self, record: &log::LogRecord) {
            if self.enabled(record.metadata()) {
                println!("{}{} [{}] {}",
                    *self.0.lock().unwrap(),
                    record.location().module_path(),
                    record.level(),
                    record.args(),
                );
            }
        }
    }

    pub fn init(prefix: Arc<Mutex<String>>) -> Result<(), log::SetLoggerError> {
        log::set_logger(|max_log_level| {
            max_log_level.set(log::LogLevelFilter::Debug);
            Box::new(SimpleLogger(prefix))
        })
    }
}

fn main() {
    let pfx = Arc::new(Mutex::new(String::new()));

    logger::init(pfx.clone()).ok().expect("failed to initialize logger");

    info!("oxensim starting!");

    let n1 = Sid::new("A__");
    let n2 = Sid::new("_B_");
    let n3 = Sid::new("__C");
    let n4 = Sid::new("dd_");
    let n5 = Sid::new("_ee");

    let mut cfg = NetConfig::complete(
        &[n1, n2, n3, n4, n5],
        0.02, // 2% packet loss between all hosts
        0.15, 0.01, // ~150ish ms latency between hosts
    );

    let mut net = NetSim::new(&cfg, pfx);
    let mut nodes = HashMap::new();
    let now = time::get_time();

    let delay = Normal::new(1000.0, 300.0);

    nodes.insert(n1, oxen(&mut net, n1, now, &delay));
    nodes.insert(n2, oxen(&mut net, n2, now, &delay));
    nodes.insert(n3, oxen(&mut net, n3, now, &delay));
    nodes.insert(n4, oxen(&mut net, n4, now, &delay));
    nodes.insert(n5, oxen(&mut net, n5, now, &delay));

    let now = run(net, nodes, now, Duration::hours(1));
}
