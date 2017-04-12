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

use futures::Future;
use futures::Poll;
use futures::Async;
use futures::task;

use tokio_core::reactor::Handle;
use tokio_io::AsyncWrite;

use irc;

#[derive(Clone)]
pub struct SendPool {
    inner: Rc<RefCell<SendPoolInner>>,
}

struct SendPoolInner {
    next_id: u64,
    pool: HashMap<u64, Sender>,
}

impl SendPool {
    pub fn new() -> SendPool {
        let inner = SendPoolInner {
            next_id: 1,
            pool: HashMap::new(),
        };

        SendPool { inner: Rc::new(RefCell::new(inner)) }
    }

    pub fn insert(&self, out: Sender) -> u64 {
        self.inner.borrow_mut().insert(out)
    }

    pub fn remove(&self, id: u64) {
        self.inner.borrow_mut().remove(id);
    }

    pub fn send_all(&self, buf: &[u8]) {
        self.inner.borrow_mut().send_all(buf);
    }
}

impl SendPoolInner {
    fn insert(&mut self, out: Sender) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pool.insert(id, out);
        id
    }

    fn remove(&mut self, id: u64) {
        self.pool.remove(&id);
    }

    fn send_all(&mut self, buf: &[u8]) {
        for out in self.pool.values_mut() {
            out.send(buf);
        }
    }
}

struct SendInner {
    next_buf: BytesMut,
    status: SendStatus,
    blocked_send: Option<task::Task>,
}

#[derive(Eq, PartialEq)]
enum SendStatus {
    Writable,
    Draining,
    StopImmediately,
}

impl Drop for SendInner {
    fn drop(&mut self) {
        info!("(SendInner) I am forgotten...");
    }
}

#[derive(Clone)]
pub struct Sender {
    inner: Weak<RefCell<SendInner>>,
}

impl Sender {
    pub fn bind<W: 'static>(handle: &Handle, send: W) -> Sender where W: AsyncWrite {
        let driver = Driver::new(send);
        let inner = Rc::downgrade(&driver.inner);
        handle.spawn(driver);
        Sender { inner: inner }
    }

    pub fn send(&mut self, buf: &[u8]) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();

            if inner.status == SendStatus::Writable {
                inner.next_buf.reserve(buf.len());
                inner.next_buf.put(buf);
            } else {
                warn!("silently discarding write of {} bytes", buf.len());
            }

            // TODO: awake the thread even on discarded writes?
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("send() on completed Sender");
        }
    }

    pub fn close_soft(&mut self) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();
            if inner.status == SendStatus::Writable {
                inner.status = SendStatus::Draining;
            }
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("close_soft() on completed Sender");
        }
    }

    pub fn close_hard(&mut self) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();
            inner.status = SendStatus::StopImmediately;
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("close_hard() on completed Sender");
        }
    }
}

struct Driver<W> {
    send: W,
    state: DriverState,
    inner: Rc<RefCell<SendInner>>,
}

enum DriverState {
    Empty,
    Parking(BytesMut),
    Parked(BytesMut),
    Draining(io::Cursor<BytesMut>),
}

impl<W: AsyncWrite> Driver<W> {
    fn new(send: W) -> Driver<W> {
        // TODO: revisit 64
        let buf1 = BytesMut::with_capacity(64);
        let buf2 = BytesMut::with_capacity(64);

        let inner = SendInner {
            next_buf: buf1,
            status: SendStatus::Writable,
            blocked_send: None,
        };

        Driver {
            send: send,
            state: DriverState::Parked(buf2),
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    fn poll_error(&mut self) -> Poll<(), irc::Error> {
        for _ in 0..50 {
            let mut inner = self.inner.borrow_mut();

            if inner.status == SendStatus::StopImmediately {
                return Ok(Async::Ready(()));
            }

            match mem::replace(&mut self.state, DriverState::Empty) {
                DriverState::Empty => {
                    return Err(irc::Error::Other("send driver internal error"));
                },

                DriverState::Parking(buf) => {
                    if inner.status == SendStatus::Draining {
                        return Ok(Async::Ready(()));
                    } else {
                        inner.blocked_send = Some(task::park());
                        self.state = DriverState::Parked(buf);
                        return Ok(Async::NotReady);
                    }
                },

                DriverState::Parked(mut buf) => {
                    if inner.next_buf.len() > 0 {
                        buf.clear();
                        mem::swap(&mut buf, &mut inner.next_buf);
                        self.state = DriverState::Draining(io::Cursor::new(buf));
                    } else {
                        self.state = DriverState::Parking(buf);
                    }
                },

                DriverState::Draining(mut buf) => {
                    // TODO: check buf has bytes to send

                    if let Async::Ready(n) = try!(self.send.write_buf(&mut buf)) {
                        if n == 0 {
                            return Ok(Async::Ready(()));
                        }
                    } else {
                        self.state = DriverState::Draining(buf);
                        return Ok(Async::NotReady);
                    }

                    if buf.has_remaining() {
                        self.state = DriverState::Draining(buf);
                    } else {
                        self.state = DriverState::Parking(buf.into_inner());
                    }
                },
            }

            drop(inner);
        }

        warn!("a driver appears to be spinning");

        // "yield" to allow other tasks to make progress
        task::park().unpark();
        Ok(Async::NotReady)
    }
}

impl<W: AsyncWrite> Future for Driver<W> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.poll_error().map_err(|e| warn!("driver errored: {}", e))
    }
}

impl<W> Drop for Driver<W> {
    fn drop(&mut self) {
        debug!("driver finished");
    }
}
