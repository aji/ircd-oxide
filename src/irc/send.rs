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

use tokio_io::AsyncWrite;

use irc::ClientError;

#[derive(Clone)]
pub struct SendPool {
    inner: Rc<RefCell<SendPoolInner>>,
}

struct SendPoolInner {
    next_id: u64,
    pool: HashMap<u64, SendHandle>,
}

impl SendPool {
    pub fn new() -> SendPool {
        let inner = SendPoolInner {
            next_id: 1,
            pool: HashMap::new(),
        };

        SendPool { inner: Rc::new(RefCell::new(inner)) }
    }

    pub fn insert(&self, out: SendHandle) -> u64 {
        self.inner.borrow_mut().insert(out)
    }

    pub fn remove(&self, id: u64) {
        self.inner.borrow_mut().remove(id);
    }

    pub fn send_all<T: IntoBuf>(&self, data: T) {
        self.inner.borrow_mut().send_all(data);
    }
}

impl SendPoolInner {
    fn insert(&mut self, out: SendHandle) -> u64 {
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
pub struct SendHandle {
    inner: Weak<RefCell<SendInner>>,
}

pub struct SendBinding<W> {
    send: W,
    state: SendState,
    inner: Rc<RefCell<SendInner>>,
}

enum SendState {
    Empty,
    Parking,
    Parked,
    Draining(io::Cursor<BytesMut>),
}

impl SendHandle {
    pub fn send<T: IntoBuf>(&mut self, into_buf: T) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();

            let buf = into_buf.into_buf();
            if inner.status == SendStatus::Writable {
                inner.next_buf.reserve(buf.remaining());
                inner.next_buf.put(buf);
            } else {
                warn!("silently discarding write of {} bytes", buf.remaining());
            }

            // TODO: awake the thread even on discarded writes?
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("send() on completed SendBinding");
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
            warn!("close_soft() on completed SendBinding");
        }
    }

    pub fn close_hard(&mut self) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();
            inner.status = SendStatus::StopImmediately;
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("close_hard() on completed SendBinding");
        }
    }
}

impl<W> SendBinding<W> {
    pub fn new(send: W) -> SendBinding<W> {
        let inner = SendInner {
            next_buf: BytesMut::with_capacity(64), // TODO: revisit 64
            status: SendStatus::Writable,
            blocked_send: None,
        };

        SendBinding {
            send: send,
            state: SendState::Parked,
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    pub fn handle(&self) -> SendHandle {
        SendHandle { inner: Rc::downgrade(&self.inner) }
    }
}

impl<W: AsyncWrite> Future for SendBinding<W> {
    type Item = ();
    type Error = ClientError;

    fn poll(&mut self) -> Poll<(), ClientError> {
        info!("poll write binding");

        for _ in 0..50 {
            let mut inner = self.inner.borrow_mut();

            if inner.status == SendStatus::StopImmediately {
                return Ok(Async::Ready(()));
            }

            match mem::replace(&mut self.state, SendState::Empty) {
                SendState::Empty => panic!("cannot poll SendBinding while empty"),

                SendState::Parking => {
                    if inner.status == SendStatus::Draining {
                        info!("drained! stopping writer");
                        return Ok(Async::Ready(()));
                    } else {
                        inner.blocked_send = Some(task::park());
                        info!("parking -> parked");
                        self.state = SendState::Parked;
                        return Ok(Async::NotReady);
                    }
                },

                SendState::Parked => {
                    if inner.next_buf.len() > 0 {
                        let mut next = BytesMut::with_capacity(64); // TODO: revisit 64
                        mem::swap(&mut next, &mut inner.next_buf);
                        info!("parked -> drain ({} bytes)", next.len());
                        self.state = SendState::Draining(io::Cursor::new(next));
                    } else {
                        info!("parked -> parking");
                        self.state = SendState::Parking;
                    }
                },

                SendState::Draining(mut buf) => {
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
                            self.state = SendState::Draining(buf);
                            return Ok(Async::NotReady);
                        },
                        Err(_) => {
                            info!("drain error");
                            return Err(ClientError::Other("drain error"));
                        },
                    }

                    if buf.has_remaining() {
                        info!("drain -> drain");
                        self.state = SendState::Draining(buf);
                    } else {
                        info!("drain -> parking");
                        self.state = SendState::Parking;
                    }
                },
            }

            drop(inner);
        }

        // "yield" to allow other tasks to make progress
        task::park().unpark();
        Ok(Async::NotReady)
    }
}

impl<W> Drop for SendBinding<W> {
    fn drop(&mut self) {
        info!("(SendBinding) I am forgotten...");
    }
}
