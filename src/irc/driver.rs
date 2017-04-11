use std::mem;

use futures::Future;
use futures::Poll;
use futures::Async;
use futures::task;
use futures::Stream;

use irc::ClientError;
use irc::message::Message;
use irc::pluto::Pluto;

pub trait State: Sized {
    type Next;

    fn handle(self, pluto: Pluto, m: Message) -> ClientOp<Self>;

    fn transition(self) -> Result<Self::Next, Self>;

    fn handle_eof(self, _pluto: Pluto) -> ClientOp<Self> {
        ClientOp::err(ClientError::Other("unexpected EOF"))
    }

    fn driver<R>(self, pluto: Pluto, recv: R) -> Driver<Self, R> {
        Driver {
            pluto: pluto,
            seen_eof: false,
            state: DriverState::Ready(self, recv),
        }
    }
}

pub struct Driver<S: State, R> {
    pluto: Pluto,
    seen_eof: bool,
    state: DriverState<S, R>,
}

enum DriverState<S: State, R> {
    Empty,
    Ready(S, R),
    Processing(ClientOp<S>, R),
}

impl<S: State, R> Driver<S, R> {
    pub fn new(state: S, pluto: Pluto, recv: R) -> Driver<S, R> {
        Driver {
            pluto: pluto,
            seen_eof: false,
            state: DriverState::Ready(state, recv)
        }
    }
}

impl<S: State, R: Stream<Item=Message>> Future for Driver<S, R>
    where S: State,
          R: Stream<Item=Message>,
          ClientError: From<R::Error>,
{
    type Item = (S::Next, R);
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(S::Next, R), ClientError> {
        for _ in 0..50 {
            match mem::replace(&mut self.state, DriverState::Empty) {
                DriverState::Empty => {
                    error!("internal client driver error");
                    return Err(ClientError::Other("internal client driver error"));
                },

                DriverState::Ready(state, mut recv) => {
                    if self.seen_eof {
                        error!("client state appears to be waiting for more input after EOF");
                        return Err(ClientError::Other("Ready, while seen_eof"));
                    }

                    match recv.poll() {
                        Ok(Async::Ready(Some(m))) => {
                            let op = state.handle(self.pluto.clone(), m);
                            self.state = DriverState::Processing(op, recv);
                        },

                        Ok(Async::Ready(None)) => {
                            let op = state.handle_eof(self.pluto.clone());
                            self.seen_eof = true;
                            self.state = DriverState::Processing(op, recv);
                        },

                        Ok(Async::NotReady) => {
                            self.state = DriverState::Ready(state, recv);
                            return Ok(Async::NotReady);
                        },

                        Err(e) => {
                            return Err(From::from(e));
                        },
                    }
                },

                DriverState::Processing(mut op, recv) => {
                    match op.poll() {
                        Ok(Async::Ready(state)) => match state.transition() {
                            Ok(next) => return Ok(Async::Ready((next, recv))),
                            Err(state) => self.state = DriverState::Ready(state, recv),
                        },

                        Ok(Async::NotReady) => {
                            self.state = DriverState::Processing(op, recv);
                            return Ok(Async::NotReady);
                        },

                        Err(e) => {
                            return Err(e);
                        },
                    }
                },
            }
        }

        // "yield" to allow other tasks to make progress
        task::park().unpark();
        Ok(Async::NotReady)
    }
}

pub enum ClientOp<T> {
    Nil(Option<Result<T, ClientError>>),
    Boxed(Box<Future<Item=T, Error=ClientError>>)
}

impl<T> ClientOp<T> {
    pub fn ok(data: T) -> ClientOp<T> {
        ClientOp::Nil(Some(Ok(data)))
    }

    pub fn err(e: ClientError) -> ClientOp<T> {
        ClientOp::Nil(Some(Err(e)))
    }

    pub fn boxed<F>(f: F) -> ClientOp<T>
    where F: 'static + Future<Item=T, Error=ClientError> {
        ClientOp::Boxed(Box::new(f))
    }
}

impl<T> Future for ClientOp<T> {
    type Item = T;
    type Error = ClientError;

    fn poll(&mut self) -> Poll<T, ClientError> {
        match *self {
            ClientOp::Nil(ref mut inner) =>
                inner.take().expect("cannot poll ClientOp::Nil twice").map(Async::Ready),

            ClientOp::Boxed(ref mut inner) =>
                inner.poll(),
        }
    }
}
