// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use std::cell::RefCell;
use std::io;
use std::mem;
use std::rc::Rc;

use bytes::Buf;
use bytes::BytesMut;

use futures;
use futures::Future;
use futures::IntoFuture;
use futures::Poll;
use futures::Async;
use futures::task;
use futures::unsync::mpsc::UnboundedSender;

use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;

use irc::message::Message;
use irc::pluto::Pluto;
use irc::pluto::PlutoTx;
use irc::pluto::PlutoReader;
use irc::pluto::PlutoWriter;

pub struct Client {
    out: UnboundedSender<String>,
}

pub struct ClientError;

impl Client {
    pub fn new(out: UnboundedSender<String>) -> Client {
        Client { out: out }
    }

    pub fn handle(self, pluto: Pluto, m: Message) -> ClientOp {
        match &m.verb[..] {
            b"INC" => {
                let tx = pluto.tx(move |p| {
                    let next = p.get() + 1;
                    p.set(next);
                    self
                }).map_err(|_| ClientError);
                ClientOp::boxed(tx)
            },

            b"GET" => {
                self.out.send(format!("value is: {}", pluto.get())).unwrap();
                ClientOp::ok(self)
            },

            _ => {
                self.out.send(format!("no idea what you meant")).unwrap();
                ClientOp::ok(self)
            }
        }
    }
}

struct ReadBinding<R> {
    recv: R,
    recv_buf: BytesMut,
    state: ReadState,
}

enum ReadState {
    Empty,
    Ready(Client),
    Op(ClientOp),
}

impl<R: AsyncRead> Future for ReadBinding<R> {
    type Item = ();
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(), ClientError> {
        loop {
            match mem::replace(&mut self.state, ReadState::Empty) {
                ReadState::Empty => panic!("cannot poll ReadBinding while empty"),

                ReadState::Ready(client) => {
                    panic!("TODO"); // TODO
                },

                ReadState::Op(mut op) => {
                    match try!(op.poll()) {
                        Async::Ready(client) => self.state = ReadState::Ready(client),
                        Async::NotReady => {
                            self.state = ReadState::Op(op);
                            return Ok(Async::NotReady);
                        }
                    }
                },
            }
        }
    }
}

struct WriteData {
    next_buf: BytesMut,
    blocked_send: Option<task::Task>,
}

type WriteRef = Rc<RefCell<WriteData>>;

struct WriteHandle {
    data: WriteRef,
}

struct WriteBinding<W> {
    send: W,
    state: WriteState,
    data: WriteRef,
}

enum WriteState {
    Empty,
    Parking,
    Parked,
    Draining(io::Cursor<BytesMut>),
}

impl<W: AsyncWrite> Future for WriteBinding<W> {
    type Item = ();
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(), ClientError> {
        loop {
            match mem::replace(&mut self.state, WriteState::Empty) {
                WriteState::Empty => panic!("cannot poll WriteBinding while empty"),

                WriteState::Parking => {
                    self.data.borrow_mut().blocked_send = Some(task::park());
                    self.state = WriteState::Parked;
                    return Ok(Async::NotReady);
                },

                WriteState::Parked => {
                    let mut data = self.data.borrow_mut();

                    if data.next_buf.len() > 0 {
                        let mut next = BytesMut::with_capacity(64); // TODO: revisit 64
                        mem::swap(&mut next, &mut data.next_buf);
                        self.state = WriteState::Draining(io::Cursor::new(next));
                    } else {
                        self.state = WriteState::Parking;
                    }

                    drop(data);
                },

                WriteState::Draining(mut buf) => {
                    match self.send.write_buf(&mut buf) {
                        Ok(Async::Ready(_)) => { },
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Err(_) => return Err(ClientError),
                    }

                    if buf.has_remaining() {
                        self.state = WriteState::Parking;
                    } else {
                        self.state = WriteState::Draining(buf);
                    }
                },
            }
        }
    }
}

impl From<ClientError> for io::Error {
    fn from(_: ClientError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, "client error")
    }
}

pub enum ClientOp {
    Nil(Option<Result<Client, ClientError>>),
    Boxed(Box<Future<Item=Client, Error=ClientError>>)
}

impl ClientOp {
    pub fn ok(c: Client) -> ClientOp {
        ClientOp::Nil(Some(Ok(c)))
    }

    pub fn err(e: ClientError) -> ClientOp {
        ClientOp::Nil(Some(Err(e)))
    }

    pub fn boxed<F>(f: F) -> ClientOp
    where F: 'static + Future<Item=Client, Error=ClientError> {
        ClientOp::Boxed(Box::new(f))
    }
}

impl Future for ClientOp {
    type Item = Client;
    type Error = ClientError;

    fn poll(&mut self) -> Poll<Client, ClientError> {
        match *self {
            ClientOp::Nil(ref mut inner) =>
                inner.take().expect("cannot poll ClientOp::Nil twice").map(Async::Ready),

            ClientOp::Boxed(ref mut inner) =>
                inner.poll(),
        }
    }
}
