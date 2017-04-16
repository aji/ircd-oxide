// irc/mod.rs -- IRC handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide, and is protected under the terms contained
// in the COPYING file in the project root.

//! Logic for handling specifics of the IRC client protocol

pub mod active;
pub mod cap;
pub mod codec;
pub mod message;
pub mod op;
pub mod pending;
pub mod pool;
pub mod send;

use std::cmp;
use std::convert::From;
use std::fmt;
use std::io;

pub use self::message::Message;
pub use self::op::Op;
pub use self::pending::Listener;

/// A generic error type for IRC client handling. Where they occur, they generally cause the
/// client connection to be closed as soon as possible.
#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Other(&'static str),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<&'static str> for Error {
    fn from(err: &'static str) -> Error {
        Error::Other(err)
    }
}

impl From<()> for Error {
    fn from(_: ()) -> Error {
        Error::Other("(unknown error)")
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        match err {
            Error::IO(e) => e,
            Error::Other(msg) => io::Error::new(io::ErrorKind::Other, msg),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref e) => write!(f, "(io) {}", e),
            Error::Other(ref s) => write!(f, "(?) {}", s),
        }
    }
}

/// A result alias for operations that fail with an `irc::Error`
pub type Result<T> = ::std::result::Result<T, Error>;
