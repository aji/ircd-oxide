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

use irc::IRCD;
use irc::Message;
use irc::net::IrcStream;
use irc::numeric::*;
use irc::output::IrcWriter;
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
    sock: IrcStream,
    data: PendingData,
}

// simplifies command invocations
struct PendingContext<'c> {
    ircd: &'c IRCD,
    wr: IrcWriter<'c>,
    data: &'c mut PendingData,
}

impl PendingClient {
    fn new(sock: TcpStream) -> PendingClient {
        PendingClient {
            sock: IrcStream::new(sock),
            data: PendingData::new(),
        }
    }

    /// Registers the `PendingClient` with the given `EventLoop`
    pub fn register<H>(&self, tok: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        self.sock.register(tok, ev)
    }

    /// Called to indicate data is ready on the client's socket.
    pub fn ready(&mut self, ircd: &IRCD, pch: &PendingHandler)
    -> io::Result<run::Action> {
        // we have to do this because borrowck cannot split borrows across
        // closure boundaries, so we split it out here where we can.
        let sock = &self.sock;
        let data = &mut self.data;

        if self.sock.empty() {
            return Ok(run::Action::DropPeer);
        }

        let _: Option<()> = try!(self.sock.read(|ln| {
            let m = match Message::parse(ln) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mut ctx = PendingContext {
                ircd: ircd,
                wr: ircd.writer(sock),
                data: data,
            };

            debug!(" -> {}", String::from_utf8_lossy(ln));
            debug!("    {:?}", m);

            pch.handle(&mut ctx, &m);

            if ctx.data.can_promote() {
                Some(())
            } else {
                None
            }
        }));

        if data.can_promote() {
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
    cb: Box<for<'c> Fn(&mut PendingContext<'c>, &Message<'c>)>,
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
    where F: 'static + for<'c> Fn(&mut PendingContext<'c>, &Message<'c>) {
        self.handlers.entry(verb.to_vec()).or_insert_with(|| HandlerFn {
            args: args,
            cb: Box::new(func)
        });
    }

    /// Handles a message from a pending client.
    fn handle<'c>(&self, ctx: &mut PendingContext<'c>, m: &Message<'c>) {
        match self.handlers.get(m.verb) {
            Some(hdlr) => {
                if m.args.len() < hdlr.args {
                    ctx.wr.numeric(ERR_NEEDMOREPARAMS, &[m.verb]);
                } else {
                    (hdlr.cb)(ctx, m);
                }
            },

            None => {
                ctx.wr.numeric(ERR_UNKNOWNCOMMAND, &[m.verb]);
            }
        }
    }
}

// in a function so we can dedent
fn handlers(pch: &mut PendingHandler) {
    pch.add(b"CAP", 1, |ctx, _m| {
        ctx.wr.numeric(ERR_INVALIDCAPCMD, &[b"FOO"]);
        info!("capabilities!");
    });

    pch.add(b"NICK", 1, |ctx, m| {
        ctx.data.nick = Some(m.args[0].to_vec());
        info!("nickname = {:?}", ctx.data.nick);
    });

    pch.add(b"USER", 4, |ctx, m| {
        ctx.data.user = Some(m.args[0].to_vec());
        info!("username = {:?}", ctx.data.user);
    });
}
