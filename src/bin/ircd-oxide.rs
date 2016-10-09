extern crate ircd;
extern crate log;
extern crate mio;

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

    ircd::looper::run(
        ircd::top::Context::new(),
        |looper, ev| looper.add(ev, |_, ev, tk| {
            let listener = ircd::irc::listen::Listener::new(("0.0.0.0", 5050), ev, tk);
            match listener {
                Ok(l) => Ok(Box::new(l)),
                Err(e) => Err(e),
            }
        })
    ).expect("loop exited with an error");
}
