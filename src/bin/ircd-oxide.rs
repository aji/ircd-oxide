extern crate futures;
extern crate oxide;
extern crate tokio_core;
extern crate log;

mod logger {
    use log;

    struct SimpleLogger;

    impl log::Log for SimpleLogger {
        fn enabled(&self, metadata: &log::LogMetadata) -> bool {
            metadata.level() <= log::LogLevel::Debug
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
            max_log_level.set(log::LogLevelFilter::Debug);
            Box::new(SimpleLogger)
        })
    }
}

fn main() {
    logger::init().expect("failed to initialize logger");

    let mut core = tokio_core::reactor::Core::new().expect("failed to initialize Tokio");
    let addr = "127.0.0.1:6667".parse().unwrap();
    oxide::irc::server::listen(core.handle(), &addr).expect("failed to create listener");
    core.run(futures::future::empty::<(), ()>()).expect("Tokio exited");
}
