extern crate ircd;
extern crate mio;

fn main() {
    let mut daemon = ircd::run::IRCD::new();
    let mut event_loop = mio::EventLoop::new().unwrap();

    event_loop.run(&mut daemon);
}
