use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;

use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;

use irc::driver::Driver;
use world::World;

/// A task to spawn pending clients from a stream of incoming connections.
pub struct Listener<A> {
    handle: Handle,
    world: World,
    accept: A,
}

impl<A> Listener<A> {
    /// Creates a new `Listener`
    pub fn new(handle: &Handle, world: World, accept: A) -> Listener<A> {
        Listener {
            handle: handle.clone(),
            world: world,
            accept: accept,
        }
    }
}

impl<A> Future for Listener<A> where A: Stream<Item=TcpStream> {
    type Item = ();
    type Error = A::Error;

    fn poll(&mut self) -> Poll<(), A::Error> {
        loop {
            let (recv, send) = match try_ready!(self.accept.poll()) {
                Some(r) => r.split(),
                None => return Ok(Async::Ready(())),
            };

            let driver = Driver::new(self.world.clone(), recv, send);
            self.handle.spawn(driver);
        }
    }
}
