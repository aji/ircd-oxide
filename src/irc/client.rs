// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use mio;
use mio::tcp::TcpStream;
use std::collections::HashMap;
use std::io;
use take_mut;

use irc::global::IRCD;
use irc::message::Message;
use irc::net::IrcStream;
use irc::numeric::*;
use irc::output::IrcWriter;
use run;
use state::World;

/// An IRC client
pub struct Client {
    sock: IrcStream,
    state: ClientState,
}

enum ClientState {
    Pending(PendingData),
    Active(ActiveData),
}

struct PendingData;

struct ActiveData;

// Simplifies command invocations
struct ClientContext<'c> {
    ircd: &'c IRCD,
    world: &'c mut World,
    wr: IrcWriter<'c>,
}

impl Client {
    /// Wraps an `TcpStream` as a `Client`
    pub fn new(sock: TcpStream) -> Client {
        Client {
            sock: IrcStream::new(sock),
            state: ClientState::Pending(PendingData),
        }
    }

    /// Registers the `Client` with the given `EventLoop`
    pub fn register<H>(&self, tok: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        self.sock.register(tok, ev)
    }

    /// Called to indicate data is ready on the client's socket.
    pub fn ready(&mut self, ircd: &IRCD, world: &mut World, ch: &ClientHandler)
    -> io::Result<run::Action> {
        let sock = &self.sock;
        let state = &mut self.state;

        if self.sock.empty() {
            return Ok(run::Action::DropPeer);
        }

        let _: Option<()> = try!(self.sock.read(|ln| {
            let m = match Message::parse(ln) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mut ctx = ClientContext {
                ircd: ircd,
                world: world,
                wr: ircd.writer(sock),
            };

            debug!("--> {}", String::from_utf8_lossy(ln));
            debug!("    {:?}", m);

            take_mut::take(state, |state| match state {
                ClientState::Pending(mut data) => {
                    ch.pending.handle(&mut ctx, &mut data, &m);
                    ClientState::Active(ActiveData)
                },
                ClientState::Active(mut data) => {
                    ch.active.handle(&mut ctx, &mut data, &m);
                    ClientState::Pending(PendingData)
                },
            });

            None
        }));

        Ok(run::Action::Continue)
    }
}

impl From<TcpStream> for Client {
    fn from(s: TcpStream) -> Client {
        Client::new(s)
    }
}

struct HandlerFn<T> {
    args: usize,
    cb: Box<for<'c> Fn(&mut ClientContext<'c>, &mut T, &Message<'c>)>,
}

struct HandlerSet<T> {
    handlers: HashMap<Vec<u8>, HandlerFn<T>>
}

impl<T> HandlerSet<T> {
    fn new() -> HandlerSet<T> {
        HandlerSet { handlers: HashMap::new() }
    }

    fn add<F>(&mut self, verb: &[u8], args: usize, func: F)
    where F: 'static + for<'c> Fn(&mut ClientContext<'c>, &mut T, &Message<'c>) {
        self.handlers
            .entry(verb.to_vec())
            .or_insert_with(|| HandlerFn {
                args: args,
                cb: Box::new(func)
            });
    }

    fn handle<'c>(&self, ctx: &mut ClientContext<'c>, data: &mut T, m: &Message<'c>) {
        match self.handlers.get(m.verb) {
            Some(hdlr) => {
                if m.args.len() < hdlr.args {
                    ctx.wr.numeric(ERR_NEEDMOREPARAMS, &[m.verb]);
                    debug!("not enough args!");
                } else {
                    (hdlr.cb)(ctx, data, m);
                }
            },

            None => {
                ctx.wr.numeric(ERR_UNKNOWNCOMMAND, &[m.verb]);
                debug!("client used unknown command");
            }
        }
    }
}

/// A client handler.
pub struct ClientHandler {
    pending: HandlerSet<PendingData>,
    active: HandlerSet<ActiveData>,
}

impl ClientHandler {
    /// Creates a new client handling structure.
    pub fn new() -> ClientHandler {
        let mut ch = ClientHandler {
            pending: HandlerSet::new(),
            active: HandlerSet::new(),
        };

        handlers(&mut ch);

        ch
    }
}

// in a funtion so we can dedent
fn handlers(ch: &mut ClientHandler) {
    ch.pending.add(b"TEST", 0, |ctx, _, m| {
        ctx.wr.numeric(ERR_NEEDMOREPARAMS, &[b"WIDGET"]);
    });

    ch.active.add(b"INC", 0, |ctx, _, m| {
        *ctx.world.counter_mut() += 1;
        let s = format!("{}", *ctx.world.counter());
        ctx.wr.snotice(b"the counter is now %s", &[s.as_bytes()]);
    });
}
