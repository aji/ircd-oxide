// irc/pending.rs -- Pending client handlers
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Pending client handlers

use mio;
use mio::tcp::TcpStream;
use std::collections::HashMap;
use std::convert::From;
use std::io;
use std::io::prelude::*;
use std::mem;

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

    /// Called to indicate data is ready on the client's socket.
    pub fn ready(&mut self) {
        let mut buf: [u8; 2048] = unsafe { mem::uninitialized() };
        let len = self.sock.read(&mut buf).expect("client read");

        let _: Option<()> = self.lb.split(&buf[..len], |ln| {
            info!(" -> {}", String::from_utf8_lossy(ln));
            None
        });
    }
}

impl From<TcpStream> for PendingClient {
    fn from(s: TcpStream) -> PendingClient {
        PendingClient::new(s)
    }
}

type HandlerFn = Box<for<'c> Fn(&mut PendingData, &Message<'c>) -> Option<()>>;

/// A pending client handler.
pub struct PendingHandler {
    handlers: HashMap<Vec<u8>, HandlerFn>,
}

impl PendingHandler {
    /// Creates a new pending client handling structure.
    pub fn new() -> PendingHandler {
        PendingHandler {
            handlers: HashMap::new()
        }
    }

    /// Handles a message from a pending client.
    fn handle<'c>(&self, ctx: &'c mut PendingData, m: Message<'c>) {
    }
}
