use futures::Async;
use futures::Future;
use futures::Poll;

use irc;

pub enum Op<T> {
    Nil(Option<irc::Result<T>>),
    Boxed(Box<Future<Item=T, Error=irc::Error>>)
}

impl<T> Op<T> {
    pub fn ok(data: T) -> Op<T> { Op::Nil(Some(Ok(data))) }

    pub fn err(e: irc::Error) -> Op<T> { Op::Nil(Some(Err(e))) }

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
