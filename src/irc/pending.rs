use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;
use futures::task;

use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::codec::FramedRead;

use irc;
use irc::pluto::Pluto;
use irc::active::Active;
use irc::codec::IrcCodec;

struct Pending {
    pluto: Pluto,
    counter: usize,
}

impl Pending {
    fn new(pluto: Pluto) -> Pending {
        Pending {
            pluto: pluto,
            counter: 0,
        }
    }

    fn bind<S: 'static>(self, handle: &Handle, sock: S)
        where S: Stream<Item=irc::Message>,
              irc::Error: From<S::Error>,
    {
        handle.spawn(Driver::new(handle, self, sock));
    }

    fn handle(mut self, m: irc::Message) -> irc::Result<Promotion> {
        info!(" -> {:?}", m);

        if b"REGISTER" == &m.verb[..] {
            self.counter += 1;
        }

        if self.counter >= 3 {
            info!("can become active");
            let active = Active::new(self.pluto);
            Ok(Promotion::Ready(irc::Op::ok(Ok(active))))
        } else {
            Ok(Promotion::NotReady(self))
        }
    }
}

enum Promotion {
    NotReady(Pending),
    Ready(irc::Op<Result<Active, Pending>>),
}

pub struct Listener<A> {
    handle: Handle,
    pluto: Pluto,
    accept: A,
}

impl<A> Listener<A> {
    pub fn new(handle: &Handle, pluto: Pluto, accept: A) -> Listener<A> {
        Listener {
            handle: handle.clone(),
            pluto: pluto,
            accept: accept,
        }
    }
}

impl<A> Future for Listener<A> where A: Stream<Item=TcpStream> {
    type Item = ();
    type Error = A::Error;

    fn poll(&mut self) -> Poll<(), A::Error> {
        loop {
            let sock = match try_ready!(self.accept.poll()) {
                Some(r) => r,
                None => return Ok(Async::Ready(())),
            };

            let recv = FramedRead::new(sock, IrcCodec);

            let pending = Pending::new(self.pluto.clone());
            pending.bind(&self.handle, recv);
        }
    }
}

struct Driver<S> {
    handle: Handle,
    promotion: Option<Promotion>,
    sock: Option<S>,
}

impl<S> Driver<S> {
    fn new(handle: &Handle, pending: Pending, sock: S) -> Driver<S> {
        Driver {
            handle: handle.clone(),
            promotion: Some(Promotion::NotReady(pending)),
            sock: Some(sock),
        }
    }
}

impl<S: 'static> Driver<S>
    where S: Stream<Item=irc::Message>,
          irc::Error: From<S::Error>,
{
    fn poll_error(&mut self) -> Poll<(), irc::Error> {
        loop {
            let promotion = match self.promotion.take() {
                Some(promotion) => promotion,
                None => return Err(irc::Error::Other("promotion.take() was None")),
            };

            let mut sock = match self.sock.take() {
                Some(sock) => sock,
                None => return Err(irc::Error::Other("sock.take() was None")),
            };

            match promotion {
                Promotion::NotReady(pending) => {
                    let result = try!(sock.poll());
                    self.sock = Some(sock);
                    match result {
                        Async::Ready(Some(m)) => {
                            self.promotion = Some(try!(pending.handle(m)));
                        },
                        Async::Ready(None) => {
                            return Err(irc::Error::Other("end of stream"));
                        },
                        Async::NotReady => {
                            self.promotion = Some(Promotion::NotReady(pending));
                            return Ok(Async::NotReady);
                        },
                    }
                },

                Promotion::Ready(mut op) => {
                    match try!(op.poll()) {
                        Async::Ready(Ok(active)) => {
                            active.bind(&self.handle, sock);
                            return Ok(Async::Ready(()));
                        },
                        Async::Ready(Err(pending)) => {
                            self.promotion = Some(Promotion::NotReady(pending));
                        },
                        Async::NotReady => {
                            self.promotion = Some(Promotion::Ready(op));
                        },
                    }
                    self.sock = Some(sock);
                },
            }
        }
    }
}

impl<S: 'static> Future for Driver<S>
    where S: Stream<Item=irc::Message>,
          irc::Error: From<S::Error>,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.poll_error().map_err(|e| info!("active died: {}", e))
    }
}
