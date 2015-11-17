// irc/output.rs -- a module for formatting IRC messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Formatting IRC messages

use irc::numeric::Numeric;
use irc::net::IrcStream;

/// A formatter for IRC lines
pub struct IrcFormatter {
    server: Vec<u8>
}

impl IrcFormatter {
    /// Creates a new formatter using the given server name for
    /// server-originated messages.
    pub fn new(server: &[u8]) -> IrcFormatter {
        IrcFormatter { server: server.to_vec() }
    }

    /// Creates a writer to the given IRC stream that will use this IRC
    /// formatter.
    pub fn writer<'w, 'fmt, 'sock>(&'fmt self, sock: &'sock IrcStream)
    -> IrcWriter<'w> where 'fmt: 'w, 'sock: 'w {
        IrcWriter::new(self, sock)
    }
}

/// A writer to an IRC stream, derived from an IRC formatter
pub struct IrcWriter<'w> {
    fmt: &'w IrcFormatter,
    sock: &'w IrcStream,
}

impl<'w> IrcWriter<'w> {
    fn new(fmt: &'w IrcFormatter, sock: &'w IrcStream) -> IrcWriter<'w> {
        IrcWriter { fmt: fmt, sock: sock }
    }
}
