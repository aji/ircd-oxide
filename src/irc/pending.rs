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

use irc::IRCD;
use irc::LineBuffer;
use irc::Message;
use run;

struct PendingData {
    nick: Option<Vec<u8>>,
    user: Option<Vec<u8>>,
}

impl PendingData {
    fn new() -> PendingData {
        PendingData {
            nick: None,
            user: None,
        }
    }

    fn can_promote(&self) -> bool {
        self.nick.is_some() &&
        self.user.is_some()
    }
}

/// Pending client data
pub struct PendingClient {
    sock: TcpStream,
    lb: LineBuffer,
    data: PendingData,
}

impl PendingClient {
    fn new(sock: TcpStream) -> PendingClient {
        PendingClient {
            sock: sock,
            lb: LineBuffer::new(),
            data: PendingData::new(),
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
    pub fn ready(&mut self, ircd: &IRCD, pch: &PendingHandler)
    -> io::Result<run::Action> {
        let mut buf: [u8; 2048] = unsafe { mem::uninitialized() };
        let len = try!(self.sock.read(&mut buf));

        if len == 0 {
            return Ok(run::Action::DropPeer);
        }

        // we have to do this because borrowck cannot split borrows across
        // closure boundaries, so we split it out here where we can.
        let ctx = &mut self.data;

        let _: Option<()> = self.lb.split(&buf[..len], |ln| {
            let m = match Message::parse(ln) {
                Ok(m) => m,
                Err(_) => return None,
            };

            debug!(" -> {}", String::from_utf8_lossy(ln));
            debug!("    {:?}", m);

            pch.handle(ircd, ctx, &m);

            if ctx.can_promote() {
                Some(())
            } else {
                None
            }
        });

        if ctx.can_promote() {
            Ok(run::Action::DropPeer)
        } else {
            Ok(run::Action::Continue)
        }
    }
}

impl From<TcpStream> for PendingClient {
    fn from(s: TcpStream) -> PendingClient {
        PendingClient::new(s)
    }
}

// make sure to keep this in sync with the constraint on `PendingHandler::add`.
struct HandlerFn {
    args: usize,
    cb: Box<for<'c> Fn(&IRCD, &mut PendingData, &Message<'c>)>,
}

/// A pending client handler.
pub struct PendingHandler {
    handlers: HashMap<Vec<u8>, HandlerFn>,
}

impl PendingHandler {
    /// Creates a new pending client handling structure.
    pub fn new() -> PendingHandler {
        let mut pch = PendingHandler {
            handlers: HashMap::new()
        };

        handlers(&mut pch);

        pch
    }

    /// Adds a handler function. If a handler is already defined for the given
    /// verb, nothing is added.
    fn add<F>(&mut self, verb: &[u8], args: usize, func: F)
    where F: 'static + for<'c> Fn(&IRCD, &mut PendingData, &Message<'c>) {
        self.handlers.entry(verb.to_vec()).or_insert_with(|| HandlerFn {
            args: args,
            cb: Box::new(func)
        });
    }

    /// Handles a message from a pending client.
    fn handle<'c>(&self, ircd: &IRCD, ctx: &'c mut PendingData, m: &Message<'c>) {
        match self.handlers.get(m.verb) {
            Some(hdlr) => {
                if m.args.len() < hdlr.args {
                    debug!("not enough args!");
                } else {
                    (hdlr.cb)(ircd, ctx, m);
                }
            },

            None => {
                debug!("pending client used unknown command");
            }
        }
    }
}

// in a function so we can dedent
fn handlers(pch: &mut PendingHandler) {
    pch.add(b"CAP", 1, |_ircd, _ctx, _m| {
        info!("capabilities!");
    });

    pch.add(b"NICK", 1, |_ircd, ctx, m| {
        ctx.nick = Some(m.args[0].to_vec());
        info!("nickname = {:?}", ctx.nick);
    });

    pch.add(b"USER", 4, |_ircd, ctx, m| {
        ctx.user = Some(m.args[0].to_vec());
        info!("username = {:?}", ctx.user);
    });
}
