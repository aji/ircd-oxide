// irc/pending.rs -- Pending client handlers
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Pending client handlers

use mio;
use mio::tcp::TcpStream;
use std::convert::From;
use std::io;

use irc::CommandSet;
use irc::LineBuffer;
use irc::Message;

/// Pending client data
pub struct PendingClient {
    sock: TcpStream,
    lb: LineBuffer,
    data: PendingData,
}

struct PendingData;

impl PendingClient {
    fn new(sock: TcpStream) -> PendingClient {
        PendingClient {
            sock: sock,
            lb: LineBuffer::new(),
            data: PendingData
        }
    }

    /// Registers the `PendingClient` with the given `EventLoop`
    pub fn register<H>(&self, tok: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        ev.register_opt(
            &self.sock,
            tok,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        )
    }
}

impl From<TcpStream> for PendingClient {
    fn from(s: TcpStream) -> PendingClient {
        PendingClient::new(s)
    }
}

/// A pending client handler.
pub struct PendingHandler<'c> {
    cmds: CommandSet<(&'c mut PendingClient, Message<'c>), ()>
}

impl<'c> PendingHandler<'c> {
    /// Creates a new pending client handling structure.
    pub fn new() -> PendingHandler<'c> {
        PendingHandler {
            cmds: CommandSet::new()
        }
    }

    /// Handles a message from a pending client.
    pub fn handle(&self, ctx: &'c mut PendingClient, m: Message<'c>) {
        self.cmds.handle(m.verb, (ctx, m));
    }
}
