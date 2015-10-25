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
use ircd::oxen::{Oxen, OxenHandler, Timer};

struct Event {
    to: Sid,
    at: Timespec,
    ty: EventType
}

enum EventType {
    Packet(PendingPacket),
    Timer(PendingTimer),
}

struct PendingPacket {
    from: Sid,
    data: Vec<u8>,
}

struct PendingTimer {
    token: Timer,
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.at.partial_cmp(&other.at).map(|o| o.reverse())
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.at.cmp(&other.at).reverse()
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
    }
}

impl Eq for Event { }

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

    events: BinaryHeap<Event>,
    canceled_timers: HashSet<Timer>,

    config: &'cfg NetConfig,
}

impl<'cfg> NetSim<'cfg> {
    fn new(config: &'cfg NetConfig, pfx: Arc<Mutex<String>>) -> NetSim<'cfg> {
        NetSim {
            log_prefix: pfx,

            events: BinaryHeap::new(),
            canceled_timers: HashSet::new(),

            config: config,
        }
    }

    fn prefix_set(&mut self, now: Timespec, peer: Sid) {
        self.log_prefix.lock()
            .map(|mut s| {
                *s = format!(
                    "{}.{:03} {}: ",
                    now.sec,
                    now.nsec / 1000000,
                    peer
                );
            })
            .unwrap();
    }

    fn prefix_clear(&mut self, now: Timespec) {
        self.log_prefix.lock()
            .map(|mut s| {
                *s = format!(
                    "{}.{:03}: ",
                    now.sec,
                    now.nsec / 1000000,
                );
            })
            .unwrap();
    }

    fn with_prefix<F, T>(&mut self, now: Timespec, peer: Sid, mut f: F) -> T
    where F: FnOnce(&mut NetSim<'cfg>) -> T {
        self.prefix_set(now, peer);
        let x = f(self);
        self.prefix_clear(now);
        x
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
        self.events.push(Event {
            to: to,
            at: now + latency,
            ty: EventType::Packet(PendingPacket {
                from: from,
                data: data
            }),
        });
    }

    fn queue_timer(&mut self, fire: Timespec, on: Sid, tok: Timer) {
        self.events.push(Event {
            to: on,
            at: fire,
            ty: EventType::Timer(PendingTimer {
                token: tok
            }),
        });
    }

    fn cancel_timer(&mut self, tok: Timer) {
        self.canceled_timers.insert(tok);
    }

    fn next_event(&mut self) -> Option<Event> {
        loop {
            let next_event = match self.events.pop() {
                Some(ev) => ev,
                None => return None,
            };

            if let EventType::Timer(ref timer) = next_event.ty {
                if self.canceled_timers.remove(&timer.token) {
                    continue;
                }
            }

            return Some(next_event);
        }
    }
}

struct BackSim<'r, 'ns: 'r> {
    sim: &'r mut NetSim<'ns>,
    now: Timespec,
    me: Sid,
}

impl<'r, 'ns> OxenHandler for BackSim<'r, 'ns> {
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

    sim.with_prefix(now, peer, |sim| {
        let mut back = BackSim {
            sim: sim,
            now: now + Duration::milliseconds(del as i64),
            me: peer,
        };

        Oxen::new(&mut back)
    })
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
                let now = now + Duration::milliseconds(del as i64);

                sim.with_prefix(now, *k, |sim| {
                    let mut back = BackSim {
                        sim: sim,
                        now: now,
                        me: *k,
                    };

                    q.add_peer(&mut back, *p);
                });
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

        now = evt.at;

        if let Some(n) = nodes.get_mut(&evt.to) {
            sim.with_prefix(evt.at, evt.to, move |sim| {
                let mut back = BackSim {
                    sim: sim,
                    now: now,
                    me: evt.to,
                };

                match evt.ty {
                    EventType::Packet(p) => n.incoming(&mut back, p.from, p.data),
                    EventType::Timer(t) => n.timeout(&mut back, t.token),
                }
            });
        }

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

    info!("oxensim starting!");
    let now = run(net, nodes, now, Duration::hours(1));
}
