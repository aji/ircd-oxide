use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;

use tokio_core::reactor::Handle;

use irc;
use irc::pluto::Pluto;

pub struct Active {
    pluto: Pluto
}

impl Active {
    pub fn new(pluto: Pluto) -> Active {
        Active { pluto: pluto }
    }

    pub fn bind<S: 'static>(self, handle: &Handle, sock: S)
        where S: Stream<Item=irc::Message>,
              irc::Error: From<S::Error>
    {
        handle.spawn(Driver::new(self, sock));
    }

    fn handle(self, m: irc::Message) -> irc::Op<Active> {
        info!(" -> {:?}", m);
        irc::Op::ok(self)
    }
}

struct Driver<S> {
    state: Option<DriverState>,
    sock: S,
}

enum DriverState {
    Ready(Active),
    Processing(irc::Op<Active>),
}

impl<S> Driver<S> {
    fn new(active: Active, sock: S) -> Driver<S> {
        Driver {
            state: Some(DriverState::Ready(active)),
            sock: sock,
        }
    }
}

impl<S: 'static> Driver<S>
    where S: Stream<Item=irc::Message>,
          irc::Error: From<S::Error>
{
    fn poll_error(&mut self) -> Poll<(), irc::Error> {
        loop {
            let state = match self.state.take() {
                Some(state) => state,
                None => return Err(irc::Error::Other("state.take() was None")),
            };

            match state {
                DriverState::Ready(active) => {
                    match try!(self.sock.poll()) {
                        Async::Ready(Some(message)) => {
                            let op = active.handle(message);
                            self.state = Some(DriverState::Processing(op));
                        },
                        Async::Ready(None) => {
                            return Err(irc::Error::Other("unexpected EOF"));
                        },
                        Async::NotReady => {
                            self.state = Some(DriverState::Ready(active));
                            return Ok(Async::NotReady);
                        },
                    }
                },

                DriverState::Processing(mut op) => {
                    if let Async::Ready(active) = try!(op.poll()) {
                        self.state = Some(DriverState::Ready(active));
                    } else {
                        self.state = Some(DriverState::Processing(op));
                        return Ok(Async::NotReady);
                    }
                },
            }
        }
    }
}

impl<S: 'static> Future for Driver<S>
    where S: Stream<Item=irc::Message>,
          irc::Error: From<S::Error>
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.poll_error().map_err(|e| info!("active died: {}", e))
    }
}
