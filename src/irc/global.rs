// irc/global.rs -- global types
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! "Global" types.
//!
//! Although we don't store these as global variables, in the traditional sense,
//! they're structures that many parts of the IRC handling infrastructure need
//! access to, and so we pass them around.

use irc::output::IrcFormatter;
use irc::output::IrcWriter;
use irc::net::IrcStream;

/// The top level IRC server type
pub struct IRCD {
    fmt: IrcFormatter,
}

impl IRCD {
    /// Creates a new `IRCD`
    pub fn new() -> IRCD {
        IRCD { fmt: IrcFormatter::new(b"oxide.irc") }
    }

    /// Creates an `IrcWriter` for the given `IrcStream`
    pub fn writer<'w, 'ircd, 'sock>(&'ircd self, sock: &'sock IrcStream)
    -> IrcWriter<'w> where 'ircd: 'w, 'sock: 'w {
        self.fmt.writer(None, sock)
    }
}
