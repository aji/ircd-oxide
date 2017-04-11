// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::mem;
use std::rc::Rc;
use std::rc::Weak;
use std::time::Duration;

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
use futures::Stream;

use tokio_core::reactor::Handle;
use tokio_core::reactor::Timeout;
use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;
use tokio_io::codec::FramedRead;

use irc::codec::IrcCodec;
use irc::message::Message;
use irc::pluto::Pluto;
use irc::pluto::PlutoTx;
use irc::pluto::PlutoReader;
use irc::pluto::PlutoWriter;

pub struct ClientPool {
    handle: Handle,
    pluto: Pluto,
    out: SendPool,
}

impl ClientPool {
    pub fn new(handle: Handle, pluto: Pluto) -> ClientPool {
        let out = SendPool::new();
        let inner_out = out.clone();

        let observer = pluto.observer().for_each(move |ev| {
            info!("pluto update, val = {}, waiting 1ms...", ev);
            inner_out.send_all(format!("ATTN: value is now {}\r\n", ev));
            Ok(())
        });

        handle.spawn(observer);

        ClientPool {
            handle: handle,
            pluto: pluto,
            out: out,
        }
    }

    pub fn bind<R, W>(&mut self, recv: R, send: W)
        where R: 'static + AsyncRead,
              W: 'static + AsyncWrite
    {
        let out = self.out.clone();

        let recv_binding = FramedRead::new(recv, IrcCodec);
        let send_binding = WriteBinding::new(send);

        let id = out.insert(send_binding.writer());
        let client = Pending::new(send_binding.writer());

        let mut soft_closer = send_binding.writer();
        let mut hard_closer = send_binding.writer();

        let driver = client.driver(self.pluto.clone(), recv_binding);

        let inner_pluto = self.pluto.clone();
        self.handle.spawn(driver.and_then(move |(active, recv)| {
            active.driver(inner_pluto, recv)
        }).and_then(move |(_, _)| {
            info!("receiver finished; closing writer (soft)");
            soft_closer.send(&b"Goodbye...\r\n"[..]);
            soft_closer.close_soft();
            Ok(())
        }).map_err(move |_| {
            info!("receiver errored; closing writer (hard)");
            hard_closer.close_hard();
        }));

        self.handle.spawn(send_binding.then(move |result| {
            out.remove(id);
            result
        }).map(|_| {
            info!("sender finished; nothing to do");
        }).map_err(|_| {
            info!("sender errored; nothing to do");
        }));
    }
}

#[derive(Clone)]
struct SendPool {
    inner: Rc<RefCell<SendPoolInner>>,
}

struct SendPoolInner {
    next_id: u64,
    pool: HashMap<u64, WriteHandle>,
}

impl SendPool {
    fn new() -> SendPool {
        let inner = SendPoolInner {
            next_id: 1,
            pool: HashMap::new(),
        };

        SendPool { inner: Rc::new(RefCell::new(inner)) }
    }

    fn insert(&self, out: WriteHandle) -> u64 {
        self.inner.borrow_mut().insert(out)
    }

    fn remove(&self, id: u64) {
        self.inner.borrow_mut().remove(id);
    }

    fn send_all<T: IntoBuf>(&self, data: T) {
        self.inner.borrow_mut().send_all(data);
    }
}

impl SendPoolInner {
    fn insert(&mut self, out: WriteHandle) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pool.insert(id, out);
        id
    }

    fn remove(&mut self, id: u64) {
        self.pool.remove(&id);
    }

    fn send_all<T: IntoBuf>(&mut self, into_buf: T) {
        let bytes: Vec<u8> = into_buf.into_buf().collect();
        for out in self.pool.values_mut() {
            out.send(&bytes);
        }
    }
}

pub struct ClientError;

impl From<io::Error> for ClientError {
    fn from(_: io::Error) -> ClientError {
        ClientError
    }
}

struct Pending {
    out: WriteHandle,
    counter: usize
}

impl Pending {
    fn new(out: WriteHandle) -> Pending {
        Pending { out: out, counter: 0 }
    }
}

impl State for Pending {
    type Next = Active;

    fn handle(mut self, _pluto: Pluto, m: Message) -> ClientOp<Self> {
        info!(" -> {:?}", m);

        match &m.verb[..] {
            b"REGISTER" => {
                self.out.send(&b"registering you...\r\n"[..]);
                self.counter += 1;
            },

            b"SPECIAL" => {
                self.out.send(&b"you are not special yet\r\n"[..]);
            },

            _ => { }
        }

        ClientOp::ok(self)
    }

    fn transition(self) -> Result<Active, Pending> {
        if self.counter > 2 {
            Ok(Active::from_pending(self))
        } else {
            Err(self)
        }
    }
}

struct Active {
    out: WriteHandle,
    wants_close: bool
}

impl Active {
    fn from_pending(pending: Pending) -> Active {
        Active { out: pending.out, wants_close: false }
    }
}

impl State for Active {
    type Next = ();

    fn handle(mut self, pluto: Pluto, m: Message) -> ClientOp<Self> {
        info!(" -> {:?}", m);

        match &m.verb[..] {
            b"REGISTER" => {
                self.out.send(&b"you're already registered\r\n"[..]);
            },

            b"SPECIAL" => {
                self.out.send(&b"very special!\r\n"[..]);
                let op = pluto.tx(move |p| {
                    let next = p.get() + 1;
                    self.out.send(format!("incrementing to {}\r\n", next).as_bytes());
                    p.set(next);
                    self
                }).map(|mut client| {
                    client.out.send(&b"all done!\r\n"[..]);
                    client
                }).map_err(|_| ClientError);
                return ClientOp::boxed(op);
            },

            b"CLOSE" => {
                self.wants_close = true;
            },

            _ => { }
        }

        ClientOp::ok(self)
    }

    fn handle_eof(mut self, _pluto: Pluto) -> ClientOp<Self> {
        self.wants_close = true;
        ClientOp::ok(self)
    }

    fn transition(mut self) -> Result<(), Active> {
        if self.wants_close {
            self.out.send(&b"closing you...\r\n"[..]);
            Ok(())
        } else {
            Err(self)
        }
    }
}

trait State: Sized {
    type Next;

    fn handle(self, pluto: Pluto, m: Message) -> ClientOp<Self>;

    fn transition(self) -> Result<Self::Next, Self>;

    fn handle_eof(self, _pluto: Pluto) -> ClientOp<Self> {
        ClientOp::err(ClientError)
    }

    fn driver<R>(self, pluto: Pluto, recv: R) -> Driver<Self, R> {
        Driver {
            pluto: pluto,
            seen_eof: false,
            state: DriverState::Ready(self, recv),
        }
    }
}

struct Driver<S: State, R> {
    pluto: Pluto,
    seen_eof: bool,
    state: DriverState<S, R>,
}

enum DriverState<S: State, R> {
    Empty,
    Ready(S, R),
    Processing(ClientOp<S>, R),
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
                    return Err(ClientError);
                },

                DriverState::Ready(state, mut recv) => {
                    if self.seen_eof {
                        error!("client state appears to be waiting for more input after EOF");
                        return Err(ClientError);
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
            warn!("send() on completed WriteBinding");
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

        for _ in 0..50 {
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
                            self.state = WriteState::Draining(buf);
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

        // "yield" to allow other tasks to make progress
        task::park().unpark();
        Ok(Async::NotReady)
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
