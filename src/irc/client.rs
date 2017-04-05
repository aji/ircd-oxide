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
use std::rc::Weak;

use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use bytes::IntoBuf;

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
use tokio_io::codec::Decoder;

use irc::codec::IrcCodec;
use irc::message::Message;
use irc::pluto::Pluto;
use irc::pluto::PlutoTx;
use irc::pluto::PlutoReader;
use irc::pluto::PlutoWriter;

pub struct ClientPool {
    handle: Handle,
    pluto: Pluto,
}

impl ClientPool {
    pub fn new(handle: Handle, pluto: Pluto) -> ClientPool {
        ClientPool {
            handle: handle,
            pluto: pluto,
        }
    }

    pub fn bind<R, W>(&mut self, recv: R, send: W)
        where R: 'static + AsyncRead,
              W: 'static + AsyncWrite
    {
        let send_binding = WriteBinding::new(send);
        let client = Client::new(send_binding.writer());
        let recv_binding = ReadBinding::new(recv, self.pluto.clone(), client);

        let mut soft_closer = send_binding.writer();
        let mut hard_closer = send_binding.writer();

        self.handle.spawn(recv_binding.map(move |_| {
            info!("receiver finished; closing writer (soft)");
            soft_closer.send(&b"Goodbye...\r\n"[..]);
            soft_closer.close_soft();
        }).map_err(move |_| {
            info!("receiver errored; closing writer (hard)");
            hard_closer.close_hard();
        }));

        self.handle.spawn(send_binding.map(|_| {
            info!("sender finished; nothing to do");
        }).map_err(|_| {
            info!("sender errored; nothing to do");
        }));
    }
}

pub struct Client {
    out: WriteHandle,
}

pub struct ClientError;

impl Client {
    fn new(out: WriteHandle) -> Client {
        Client { out: out }
    }

    fn handle(mut self, pluto: Pluto, m: Message) -> ClientOp {

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
                self.out.send(format!("value is: {}\n\n", pluto.get()).as_bytes());
                ClientOp::ok(self)
            },

            _ => {
                //self.out.send(format!("no idea what you meant")).unwrap();
                self.out.send(format!("no idea what you meant\r\n").as_bytes());
                ClientOp::ok(self)
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        info!("(Client) I am forgotten...");
    }
}

struct ReadBinding<R> {
    recv: R,
    recv_buf: BytesMut,
    pluto: Pluto,
    state: ReadState,
}

enum ReadState {
    Empty,
    Ready(Client),
    Lines(Client),
    Op(ClientOp),
}

impl<R> ReadBinding<R> {
    fn new(recv: R, pluto: Pluto, client: Client) -> ReadBinding<R> {
        ReadBinding {
            recv: recv,
            recv_buf: BytesMut::with_capacity(1024), // TODO: revisit 1024
            pluto: pluto,
            state: ReadState::Ready(client),
        }
    }
}

impl<R: AsyncRead> Future for ReadBinding<R> {
    type Item = ();
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(), ClientError> {
        info!("poll read binding");
        loop {
            match mem::replace(&mut self.state, ReadState::Empty) {
                ReadState::Empty => panic!("cannot poll ReadBinding while empty"),

                ReadState::Ready(client) => {
                    self.recv_buf.reserve(1);
                    match self.recv.read_buf(&mut self.recv_buf) {
                        Ok(Async::Ready(0)) => {
                            info!("EOF read");
                            return Ok(Async::Ready(()))
                        },
                        Ok(Async::Ready(n)) => {
                            info!("ready -> lines (read {} bytes)", n);
                            self.state = ReadState::Lines(client);
                        },
                        Ok(Async::NotReady) => {
                            info!("ready -> ready (not ready)");
                            self.state = ReadState::Ready(client);
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            info!("client error: {:?}", e);
                            return Err(ClientError);
                        },
                    }
                },

                ReadState::Lines(client) => {
                    match IrcCodec::decode(&mut IrcCodec, &mut self.recv_buf) {
                        Ok(Some(m)) => {
                            info!(" --> {:?}", m);
                            let op = client.handle(self.pluto.clone(), m);
                            info!("lines -> op");
                            self.state = ReadState::Op(op);
                        },
                        Ok(None) => {
                            info!("lines -> ready");
                            self.state = ReadState::Ready(client);
                        },
                        Err(e) => {
                            info!("client error: {:?}", e);
                            return Err(ClientError);
                        },
                    }
                },

                ReadState::Op(mut op) => {
                    match try!(op.poll()) {
                        Async::Ready(client) => {
                            info!("op -> lines");
                            self.state = ReadState::Lines(client);
                        },
                        Async::NotReady => {
                            info!("op -> op (not ready)");
                            self.state = ReadState::Op(op);
                            return Ok(Async::NotReady);
                        },
                    }
                },
            }
        }
    }
}

impl<R> Drop for ReadBinding<R> {
    fn drop(&mut self) {
        info!("(ReadBinding) I am forgotten...");
    }
}

struct WriteData {
    next_buf: BytesMut,
    status: WriteStatus,
    blocked_send: Option<task::Task>,
}

#[derive(Eq, PartialEq)]
enum WriteStatus {
    Writable,
    Draining,
    StopImmediately,
}

impl Drop for WriteData {
    fn drop(&mut self) {
        info!("(WriteData) I am forgotten...");
    }
}

#[derive(Clone)]
struct WriteHandle {
    data: Weak<RefCell<WriteData>>,
}

struct WriteBinding<W> {
    send: W,
    state: WriteState,
    data: Rc<RefCell<WriteData>>,
}

enum WriteState {
    Empty,
    Parking,
    Parked,
    Draining(io::Cursor<BytesMut>),
}

impl WriteHandle {
    fn send<T: IntoBuf>(&mut self, into_buf: T) {
        if let Some(r) = self.data.upgrade() {
            let mut data = r.borrow_mut();

            let buf = into_buf.into_buf();
            if data.status == WriteStatus::Writable {
                data.next_buf.reserve(buf.remaining());
                data.next_buf.put(buf);
            } else {
                warn!("silently discarding write of {} bytes", buf.remaining());
            }

            // TODO: awake the thread even on discarded writes?
            data.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("self() on completed WriteBinding");
        }
    }

    fn close_soft(&mut self) {
        if let Some(r) = self.data.upgrade() {
            let mut data = r.borrow_mut();
            if data.status == WriteStatus::Writable {
                data.status = WriteStatus::Draining;
            }
            data.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("close_soft() on completed WriteBinding");
        }
    }

    fn close_hard(&mut self) {
        if let Some(r) = self.data.upgrade() {
            let mut data = r.borrow_mut();
            data.status = WriteStatus::StopImmediately;
            data.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("close_hard() on completed WriteBinding");
        }
    }
}

impl<W> WriteBinding<W> {
    fn new(send: W) -> WriteBinding<W> {
        let data = WriteData {
            next_buf: BytesMut::with_capacity(64), // TODO: revisit 64
            status: WriteStatus::Writable,
            blocked_send: None,
        };

        WriteBinding {
            send: send,
            state: WriteState::Parked,
            data: Rc::new(RefCell::new(data)),
        }
    }

    fn writer(&self) -> WriteHandle {
        WriteHandle { data: Rc::downgrade(&self.data) }
    }
}

impl<W: AsyncWrite> Future for WriteBinding<W> {
    type Item = ();
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(), ClientError> {
        info!("poll write binding");

        loop {
            let mut data = self.data.borrow_mut();

            if data.status == WriteStatus::StopImmediately {
                return Ok(Async::Ready(()));
            }

            match mem::replace(&mut self.state, WriteState::Empty) {
                WriteState::Empty => panic!("cannot poll WriteBinding while empty"),

                WriteState::Parking => {
                    if data.status == WriteStatus::Draining {
                        info!("drained! stopping writer");
                        return Ok(Async::Ready(()));
                    } else {
                        data.blocked_send = Some(task::park());
                        info!("parking -> parked");
                        self.state = WriteState::Parked;
                        return Ok(Async::NotReady);
                    }
                },

                WriteState::Parked => {
                    if data.next_buf.len() > 0 {
                        let mut next = BytesMut::with_capacity(64); // TODO: revisit 64
                        mem::swap(&mut next, &mut data.next_buf);
                        info!("parked -> drain ({} bytes)", next.len());
                        self.state = WriteState::Draining(io::Cursor::new(next));
                    } else {
                        info!("parked -> parking");
                        self.state = WriteState::Parking;
                    }
                },

                WriteState::Draining(mut buf) => {
                    // TODO: check buf has bytes to send

                    match self.send.write_buf(&mut buf) {
                        Ok(Async::Ready(0)) => {
                            info!("EOF write");
                            return Ok(Async::Ready(()));
                        },
                        Ok(Async::Ready(n)) => {
                            info!("drained {} bytes", n);
                        },
                        Ok(Async::NotReady) => {
                            info!("drain not ready");
                            return Ok(Async::NotReady);
                        },
                        Err(_) => {
                            info!("drain error");
                            return Err(ClientError);
                        },
                    }

                    if buf.has_remaining() {
                        info!("drain -> drain");
                        self.state = WriteState::Draining(buf);
                    } else {
                        info!("drain -> parking");
                        self.state = WriteState::Parking;
                    }
                },
            }

            drop(data);
        }
    }
}

impl<W> Drop for WriteBinding<W> {
    fn drop(&mut self) {
        info!("(WriteBinding) I am forgotten...");
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
