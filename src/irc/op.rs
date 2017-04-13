//! A generic asynchronous operation

use futures::Async;
use futures::Future;
use futures::Poll;

use irc;

/// An asynchronous operation of some kind, returning a result of the given type.
///
/// This enum is similar to a `Box<Future>` (and even has such a variant) but can special-case
/// certain common operations to not require a boxed trait object, for efficiency's sake.
pub enum Op<T> {
    /// A `Nil` operation is an operation that has already finished. This is handy for when there
    /// is no asynchronous work to be done, but a value of type `Op` is still needed.
    Nil(Option<irc::Result<T>>),

    /// A `Boxed` operation is simply a wrapper around any kind of future that resolves to a `T`
    /// and errors with `irc::Error`
    Boxed(Box<Future<Item=T, Error=irc::Error>>)
}

impl<T> Op<T> {
    /// Creates an operation that succeeds immediately with the given data.
    pub fn ok(data: T) -> Op<T> { Op::Nil(Some(Ok(data))) }

    /// Creates an operation that fails immediately with the given error.
    pub fn err(e: irc::Error) -> Op<T> { Op::Nil(Some(Err(e))) }

    /// Creates an operation that wraps the given future.
    pub fn boxed<F>(f: F) -> Op<T>
    where F: 'static + Future<Item=T, Error=irc::Error> {
        Op::Boxed(Box::new(f))
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

            Op::Boxed(ref mut inner) => inner.poll(),
        }
    }
}
