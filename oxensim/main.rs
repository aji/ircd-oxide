extern crate rand;
extern crate time;

#[macro_use]
extern crate log;

extern crate ircd;

use std::collections::HashMap;
use time::Duration;

use ircd::util::Sid;
use ircd::oxen::Oxen;

mod netsim;

mod logger {
    use log;

    struct SimpleLogger;

    impl log::Log for SimpleLogger {
        fn enabled(&self, metadata: &log::LogMetadata) -> bool {
            metadata.level() <= log::LogLevel::Info
        }

        fn log(&self, record: &log::LogRecord) {
            if self.enabled(record.metadata()) {
                println!("{} [{}] {}",
                    record.location().module_path(),
                    record.level(),
                    record.args(),
                );
            }
        }
    }

    pub fn init() -> Result<(), log::SetLoggerError> {
        log::set_logger(|max_log_level| {
            max_log_level.set(log::LogLevelFilter::Info);
            Box::new(SimpleLogger)
        })
    }
}

fn main() {
    logger::init().ok().expect("failed to initialize logger");

    info!("oxensim starting!");

    let n1 = Sid::new("0N1");
    let n2 = Sid::new("0N2");
    let n3 = Sid::new("0N3");
    let n4 = Sid::new("0N4");
    let n5 = Sid::new("0N5");

    let cfg = netsim::NetConfig::complete(
        &[n1, n2, n3, n4, n5],
        0.10, // 1% packet loss between all hosts
        2.00, 1.00, // ~60ish ms latency between hosts
    );

    let mut net = netsim::NetSim::new(&cfg);

    let mut nodes = HashMap::new();
    nodes.insert(n1, Oxen::new(n1));
    nodes.insert(n2, Oxen::new(n2));
    nodes.insert(n3, Oxen::new(n3));
    nodes.insert(n4, Oxen::new(n4));
    nodes.insert(n5, Oxen::new(n5));

    net.queue_send(
        time::get_time(),
        n1, n2, b"d2:to3:0N4e".to_vec()
    );
    net.queue_send(
        time::get_time() + Duration::seconds(1),
        n1, n2, b"d2:to3:0N4e".to_vec()
    );
    net.queue_send(
        time::get_time() + Duration::seconds(2),
        n1, n3, b"d2:to3:0N3e".to_vec()
    );

    netsim::run(net, nodes, Duration::minutes(2));
}
