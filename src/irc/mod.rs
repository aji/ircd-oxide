// irc/mod.rs -- IRC handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide, and is protected under the terms contained
// in the COPYING file in the project root.

//! Logic for handling specifics of the IRC client protocol

pub mod active;
pub mod cap;
pub mod codec;
pub mod driver;
pub mod message;
pub mod pending;
pub mod pluto;
pub mod send;
pub mod server;

use std::cmp;
use std::convert::From;
use std::io;

/// An error on a client connection. These generally cause the client to be closed
#[derive(Debug)]
pub enum ClientError {
    IO(io::Error),
    Other(&'static str),
}

impl From<io::Error> for ClientError {
    fn from(err: io::Error) -> ClientError {
        ClientError::IO(err)
    }
}

impl From<&'static str> for ClientError {
    fn from(err: &'static str) -> ClientError {
        ClientError::Other(err)
    }
}

impl From<()> for ClientError {
    fn from(_: ()) -> ClientError {
        ClientError::Other("(unknown error)")
    }
}

impl From<ClientError> for io::Error {
    fn from(err: ClientError) -> io::Error {
        match err {
            ClientError::IO(e) => e,
            ClientError::Other(msg) => io::Error::new(io::ErrorKind::Other, msg),
        }
    }
}

/// An `IrcString` is a wrapper around a standard Rust `String` that provides
/// extra functionality for comparison and canonicalization based on the
/// particular casemapping (ASCII, RFC 1459, etc.) in use.
pub struct IrcString(String);

impl IrcString {
    /// Returns the canonical form of this `IrcString`. Canonical forms have the
    /// same comparison relationship as `IrcString`s (so if the canonical forms
    /// are equal, then the `IrcString`s they were derived from will be equal),
    /// but aren't otherwise too useful.
    pub fn canonicalize(&self) -> String {
        self.0.to_lowercase()
    }
}

impl From<String> for IrcString {
    fn from(s: String) -> IrcString { IrcString(s) }
}

impl From<IrcString> for String {
    fn from(s: IrcString) -> String { s.0 }
}

impl cmp::PartialEq for IrcString {
    fn eq(&self, other: &IrcString) -> bool {
        self.canonicalize().eq(&other.canonicalize())
    }

    fn ne(&self, other: &IrcString) -> bool {
        self.canonicalize().ne(&other.canonicalize())
    }
}

impl cmp::Eq for IrcString { }

impl cmp::PartialOrd for IrcString {
    fn partial_cmp(&self, other: &IrcString) -> Option<cmp::Ordering> {
        self.canonicalize().partial_cmp(&other.canonicalize())
    }
}

impl cmp::Ord for IrcString {
    fn cmp(&self, other: &IrcString) -> cmp::Ordering {
        self.canonicalize().cmp(&other.canonicalize())
    }
}

#[cfg(test)]
fn irc_string(s: &str) -> IrcString {
    From::from(s.to_owned())
}

#[test]
fn test_irc_string_eq_ne() {
    assert!(irc_string("hello") == irc_string("Hello"));
    assert!(irc_string("HELLO") == irc_string("HeLlO"));

    assert!(irc_string("hello") != irc_string("goodbye"));
}

#[test]
fn test_irc_string_cmp() {
    assert!(irc_string("foo") > irc_string("bar"));
    assert!(irc_string("FOO") > irc_string("bar"));
    assert!(irc_string("foo") > irc_string("BAR"));
    assert!(irc_string("FOO") > irc_string("BAR"));
}
