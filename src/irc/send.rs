//! Abstractions for managing the write half of sockets and collections of sockets.

// TODO: implement a way to deliver events that terminate the driver

use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

use bytes::Buf;
use bytes::BufMut;

use futures::Future;
use futures::Poll;
use futures::Async;
use futures::task;

use tokio_io::AsyncWrite;

use common::byte_ring::ByteRing;
use irc;

struct SendInner {
    buf: ByteRing,
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

/// A handle to a byte buffer, which is drained to an `AsyncWrite` in a background task.
///
/// This struct can be cheaply cloned and moved around in a single thread to make it easier to
/// push bytes to an `AsyncWrite` for asynchronous delivery. Unfortunately there is currently
/// nothing resembling backpressure, and no way to react to driver-level events such as errors
/// or termination.
#[derive(Clone)]
pub struct Sender {
    inner: Weak<RefCell<SendInner>>,
}

impl Sender {
    /// Queues some bytes up to be sent to the associated socket.
    pub fn send(&mut self, buf: &[u8]) {
        if let Some(r) = self.inner.upgrade() {
            let mut inner = r.borrow_mut();

            if inner.status == SendStatus::Writable {
                // TODO: don't panic if the buffer is full. either make room or kill the sender
                inner.buf.put(buf);
            } else {
                warn!("silently discarding write of {} bytes", buf.len());
            }

            // TODO: awake the thread even on discarded writes?
            inner.blocked_send.take().map(|t| t.unpark());
        } else {
            warn!("send() on completed Sender");
        }
    }

    /// Closes the sender for additional writes, but will continue to write any pending output
    /// to the destination until the buffers are drained.
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

    /// Closes the sender for writes, and stops the driver task on its next poll, discarding any
    /// pending output.
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

pub struct SendDriver<W> {
    send: W,
    inner: Rc<RefCell<SendInner>>
}

impl<W: AsyncWrite> SendDriver<W> {
    pub fn new(send: W) -> SendDriver<W> {
        // TODO: revisit 4096
        let buf = ByteRing::with_capacity(4096);

        let inner = SendInner {
            buf: buf,
            status: SendStatus::Writable,
            blocked_send: None,
        };

        SendDriver {
            send: send,
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    pub fn sender(&mut self) -> Sender {
        Sender { inner: Rc::downgrade(&self.inner) }
    }
}

impl<W: AsyncWrite> Future for SendDriver<W> {
    type Item = ();
    type Error = irc::Error;

    fn poll(&mut self) -> Poll<(), irc::Error> {
        let mut inner = self.inner.borrow_mut();

        if inner.status == SendStatus::StopImmediately {
            return Ok(Async::Ready(()));
        }

        while inner.buf.remaining() > 0 {
            match try!(self.send.write_buf(&mut inner.buf)) {
                Async::Ready(0) => return Err(irc::Error::Other("unexpected EOF on writer")),
                Async::Ready(_) => (), // do nothing, we can probably write more!
                Async::NotReady => break
            }
        }

        if inner.buf.remaining() == 0 && inner.status == SendStatus::Draining {
            return Ok(Async::Ready(()));
        }

        inner.blocked_send = Some(task::park());
        Ok(Async::NotReady)
    }
}
