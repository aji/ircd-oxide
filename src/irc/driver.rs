use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;
use futures::task;

use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;
use tokio_io::codec::FramedRead;

use irc;
use irc::active::Active;
use irc::codec::IrcCodec;
use irc::message::Message;
use irc::pending::Pending;
use irc::send::SendDriver;
use world::World;

pub enum Client {
    Pending(Pending),
    Active(Active),
}

impl Client {
    fn handle(self, message: Message) -> irc::Op<Client> {
        match self {
            Client::Pending(pending) => pending.handle(message),
            Client::Active(active) => active.handle(message),
        }
    }
}

pub struct Driver<R, W> {
    send: SendDriver<W>,
    recv: FramedRead<R, IrcCodec>,
    state: Option<State>,
}

enum State {
    Ready(Client),
    Processing(irc::Op<Client>),
}

type DriverPoll = Result<(State, bool), irc::Error>;

fn driver_err(e: irc::Error) -> DriverPoll { Err(e) }

fn driver_not_ready(s: State) -> DriverPoll { Ok((s, false)) }

fn driver_continue(s: State) -> DriverPoll { Ok((s, true)) }

impl<R: 'static, W: 'static> Driver<R, W>
    where R: AsyncRead,
          W: AsyncWrite,
{
    pub fn new(world: World, recv: R, send: W) -> Driver<R, W> {
        let mut send_driver = SendDriver::new(send);
        let pending = Pending::new(world, send_driver.sender());

        Driver {
            send: send_driver,
            recv: FramedRead::new(recv, IrcCodec),
            state: Some(State::Ready(Client::Pending(pending)))
        }
    }

    fn poll_driver(&mut self, state: State) -> DriverPoll {
        use self::State::*;

        match state {
            Ready(client) => {
                if let Async::Ready(result) = try!(self.recv.poll()) {
                    if let Some(message) = result {
                        let op = client.handle(message);
                        driver_continue(Processing(op))
                    } else {
                        driver_err(irc::Error::Other("unexpected EOF"))
                    }
                } else {
                    driver_not_ready(Ready(client))
                }
            },

            Processing(mut op) => {
                match try!(op.poll()) {
                    Async::Ready(client) => driver_continue(Ready(client)),
                    Async::NotReady => driver_not_ready(Processing(op)),
                }
            },
        }
    }

    fn poll_error(&mut self) -> Poll<(), irc::Error> {
        let _ = try!(self.send.poll());

        for _ in 0..50 {
            let state = match self.state.take() {
                Some(state) => state,
                None => return Err(irc::Error::Other("illegal state")),
            };

            let (next, cont) = try!(self.poll_driver(state));
            self.state = Some(next);

            if !cont {
                return Ok(Async::NotReady);
            }
        }

        warn!("a driver appears to be spinning");
        task::park().unpark();
        Ok(Async::NotReady)
    }
}

impl<R: 'static, W: 'static> Future for Driver<R, W>
    where R: AsyncRead,
          W: AsyncWrite,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.poll_error().map_err(|e| info!("driver error: {}", e))
    }
}
