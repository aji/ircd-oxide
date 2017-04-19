//! A generic asynchronous operation

use futures::Async;
use futures::Future;
use futures::Poll;

use common::observe;
use crdb;
use irc;

/// An asynchronous operation of some kind, returning a result of the given type.
///
/// This enum is similar to a `Box<Future>` (and even has such a variant) but can special-case
/// certain common operations to not require a boxed trait object, for efficiency's sake.
pub enum Op<T> {
    /// A `Nil` operation is an operation that has already finished. This is handy for when there
    /// is no asynchronous work to be done, but a value of type `Op` is still needed.
    Nil(Option<irc::Result<T>>),

    /// A future that waits for completion of a submitted observable and returns the given data
    /// when the observation has been fully consumed
    Observe(observe::Completion, Option<T>),

    /// A future that waits for completion of a CRDB transaction and returns the given data when
    /// the transaction has been fully consumed
    CRDB(crdb::Completion, Option<T>),

    /// A `Boxed` operation is simply a wrapper around any kind of future that resolves to a `T`
    /// and errors with `irc::Error`
    Boxed(Box<Future<Item=T, Error=irc::Error>>)
}

impl<T: 'static> Op<T> {
    /// Creates an operation that succeeds immediately with the given data.
    pub fn ok(data: T) -> Op<T> { Op::Nil(Some(Ok(data))) }

    /// Creates an operation that fails immediately with the given error.
    pub fn err(e: irc::Error) -> Op<T> { Op::Nil(Some(Err(e))) }

    /// Creates an operation that waits for observation completion and returns the given data.
    pub fn observe(cpl: observe::Completion, data: T) -> Op<T> {
        Op::Observe(cpl, Some(data))
    }

    /// Creates an operation that waits for CRDB completion and returns the given data.
    pub fn crdb(cpl: crdb::Completion, data: T) -> Op<T> {
        Op::CRDB(cpl, Some(data))
    }

    /// Creates an operation that wraps the given future.
    pub fn boxed<F: 'static>(f: F) -> Op<T>
    where F: Future<Item=T, Error=irc::Error> {
        Op::Boxed(Box::new(f))
    }

    /// Creates a new operation that applies the function to the result of the operation.
    pub fn map<U: 'static, F: 'static>(self, f: F) -> Op<U>
    where F: FnOnce(T) -> U {
        match self {
            Op::Nil(Some(inner)) => Op::Nil(Some(inner.map(f))),
            Op::Nil(None) => Op::Nil(None),

            Op::Observe(cpl, data) => Op::Observe(cpl, data.map(f)),
            Op::CRDB(cpl, data) => Op::CRDB(cpl, data.map(f)),

            Op::Boxed(inner) => Op::boxed(inner.map(f)),
        }
    }
}

impl<T> Future for Op<T> {
    type Item = T;
    type Error = irc::Error;

    fn poll(&mut self) -> Poll<T, irc::Error> {
        match *self {
            Op::Nil(ref mut inner) => inner.take()
                .unwrap_or(Err(irc::Error::Other("Op::Nil polled more than once")))
                .map(Async::Ready),

            // TODO: get rid of expect()s in next arms, replace with returning Err

            Op::Observe(ref mut inner, ref mut data) => {
                if inner.poll().expect("completion failed unexpectedly").is_ready() {
                    Ok(Async::Ready(data.take().expect("Op::Observe polled more than once")))
                } else {
                    Ok(Async::NotReady)
                }
            },

            Op::CRDB(ref mut inner, ref mut data) => {
                if inner.poll().expect("completion failed unexpectedly").is_ready() {
                    Ok(Async::Ready(data.take().expect("Op::CRDB polled more than once")))
                } else {
                    Ok(Async::NotReady)
                }
            },

            Op::Boxed(ref mut inner) => inner.poll(),
        }
    }
}
