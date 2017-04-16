//! Code to listen for and drive pre-registration connections

use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;
use futures::task;

use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::codec::FramedRead;
use tokio_io::AsyncRead;

use irc;
use irc::active::Active;
use irc::codec::IrcCodec;
use irc::pool::Pool;
use irc::send::Sender;

use world::World;

struct Pending {
    world: World,
    pool: Pool,
    out: Sender,
    nick: Option<String>,
}

impl Pending {
    fn new(world: World, pool: Pool, out: Sender) -> Pending {
        Pending {
            world: world,
            pool: pool,
            out: out,
            nick: None,
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

        if b"NICK" == &m.verb[..] && m.args.len() > 0 {
            if let Ok(nick) = String::from_utf8(m.args[0].to_vec()) {
                self.nick = Some(nick);
            }
        }

        if let Some(nick) = self.nick.as_ref().cloned() {
            info!("can become active");

            let op = self.world.add_user(nick.clone()).and_then(move |_| {
                info!("added user, now adding channel");
                self.pool.add_user(nick.clone(), self.out.clone());
                self.world.add_chan("#foo".to_string()).and_then(move |_| {
                    self.out.send(format!("welcome {}!\r\n", nick).as_bytes());
                    let active = Active::new(self.world, self.out, nick);
                    Ok(Ok(active))
                })
            }).map_err(|_| irc::Error::Other("register error"));

            Ok(Promotion::Ready(irc::Op::boxed(op)))

        } else {
            self.out.send(b"keep going...\r\n");
            Ok(Promotion::NotReady(self))
        }
    }
}

enum Promotion {
    NotReady(Pending),
    Ready(irc::Op<Result<Active, Pending>>),
}

/// A task to spawn pending clients from a stream of incoming connections.
pub struct Listener<A> {
    handle: Handle,
    world: World,
    pool: Pool,
    accept: A,
}

impl<A> Listener<A> {
    /// Creates a new `Listener`
    pub fn new(handle: &Handle, world: World, pool: Pool, accept: A) -> Listener<A> {
        Listener {
            handle: handle.clone(),
            world: world,
            pool: pool,
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

            let sender = Sender::bind(&self.handle, send);
            let pending = Pending::new(self.world.clone(), self.pool.clone(), sender);
            pending.bind(&self.handle, FramedRead::new(recv, IrcCodec));
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
                            self.sock = Some(sock);
                            self.promotion = Some(Promotion::NotReady(pending));
                        },
                        Async::NotReady => {
                            self.sock = Some(sock);
                            self.promotion = Some(Promotion::Ready(op));
                            return Ok(Async::NotReady);
                        },
                    }
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
