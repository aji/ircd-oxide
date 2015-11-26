// run.rs -- ircd-oxide runtime
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The runtime

use mio;
use rand::random;
use std::collections::HashMap;
use std::io;
use std::net::ToSocketAddrs;

use irc::client::Client;
use irc::client::ClientHandler;
use irc::global::IRCD;
use irc::listen::Listener;
use irc::pending::PendingClient;
use irc::pending::PendingHandler;

/// The top-level IRC server structure
pub struct Top {
    ircd: IRCD,
    tokens: HashMap<mio::Token, TokenData>,
    pch: PendingHandler,
    ch: ClientHandler,
}

enum TokenData {
    Listener(Listener),
    Pending(PendingClient),
    Client(Client),
}

/// An action to be performed by the run loop after handling an event.
pub enum Action {
    /// Do nothing
    Continue,
    /// Drop the peer that handled the event
    DropPeer,
    /// Add a pending client
    AddPending(PendingClient),
    /// Promote a connection to a regular client
    Promote,
}

impl Top {
    /// Creates a new `Top`
    pub fn new() -> Top {
        Top {
            ircd: IRCD::new(),
            tokens: HashMap::new(),
            pch: PendingHandler::new(),
            ch: ClientHandler::new(),
        }
    }

    fn add_listener(
        &mut self,
        listener: Listener,
        ev: &mut mio::EventLoop<Top>
    ) -> io::Result<()> {
        let token = mio::Token(random());
        try!(listener.register(token, ev));
        self.tokens.insert(token, TokenData::Listener(listener));
        Ok(())
    }

    fn add_pending(
        &mut self,
        pending: PendingClient,
        ev: &mut mio::EventLoop<Top>
    ) -> io::Result<()> {
        let token = mio::Token(random());
        try!(pending.register(token, ev));
        self.tokens.insert(token, TokenData::Pending(pending));
        Ok(())
    }
}

impl mio::Handler for Top {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        ev: &mut mio::EventLoop<Top>,
        tk: mio::Token,
        _events: mio::EventSet
    ) {
        debug!("event becomes ready");

        // This function is turning out to be a big mess, but the basic
        // structure is pretty straightforward:
        //
        //    let action = ...;
        //    match action { ... }
        //
        // This structure is necessary because determining what action to take
        // borrows the structures to be acted upon mutably. We have to remember
        // what we wanted to do with an Action, release our borrow, and then
        // take a new borrow to perform the Action. It's a little screwy and I'd
        // highly appreciate guidance to do it better!

        let action = {
            let tdata = match self.tokens.get_mut(&tk) {
                Some(tdata) => tdata,
                None => {
                    error!("mio woke us up with token we don't know about!");
                    return;
                }
            };

            match *tdata {
                TokenData::Listener(ref mut listener) => {
                    debug!("accepting new incoming connection");

                    match listener.accept() {
                        Ok(p) => Action::AddPending(p),

                        Err(e) => {
                            error!("accepting client: {}", e);
                            Action::Continue
                        }
                    }
                },

                TokenData::Pending(ref mut pending) => {
                    match pending.ready(&self.ircd, &self.pch) {
                        Ok(action) => action,

                        Err(e) => {
                            info!("dropping pending client: {}", e);
                            Action::DropPeer
                        }
                    }
                },

                TokenData::Client(ref mut client) => {
                    match client.ready(&self.ircd, &self.ch) {
                        Ok(action) => action,

                        Err(e) => {
                            info!("dropping client: {}", e);
                            Action::DropPeer
                        }
                    }
                },
            }
        };

        match action {
            Action::Continue => { },

            Action::DropPeer => {
                if let None = self.tokens.remove(&tk) {
                    warn!("DropPeer for token {:?} we don't have", tk);
                }
            },

            Action::AddPending(pending) => {
                if let Err(e) = self.add_pending(pending, ev) {
                    error!("error adding pending client: {}", e);
                }
            },

            Action::Promote => {
                if let Some(d) = self.tokens.remove(&tk) {
                    match d {
                        TokenData::Pending(pending) => {
                            if let Some(c) = pending.promote() {
                                self.tokens.insert(tk, TokenData::Client(c));
                            } else {
                                error!("error during promotion");
                            }
                        },

                        _ => {
                            error!("can only Promote Pending");
                            self.tokens.insert(tk, d);
                        }
                    }
                } else {
                    warn!("Promote for token {:?} we don't have", tk);
                }
            },
        }
    }
}

/// A structure for running an `Top`
pub struct Runner {
    top: Top,
    ev: mio::EventLoop<Top>,
}

impl Runner {
    /// Creates a new `Runner`
    pub fn new() -> io::Result<Runner> {
        Ok(Runner {
            top: Top::new(),
            ev: try!(mio::EventLoop::new()),
        })
    }

    /// Gets a reference to the `Top`
    pub fn top(&mut self) -> &mut Top {
        &mut self.top
    }

    /// Adds an IRC listener on the given port
    pub fn listen<A: ToSocketAddrs>(&mut self, addr: A) -> io::Result<()> {
        let listener = {
            let mut addrs = try!(ToSocketAddrs::to_socket_addrs(&addr));
            let addr = match addrs.nth(0) {
                Some(addr) => addr,
                None => panic!("help!"),
            };
            try!(mio::tcp::TcpListener::bind(&addr))
        };

        self.top.add_listener(Listener::new(listener), &mut self.ev)
    }

    /// Runs the `Runner` forever
    pub fn run(&mut self) {
        info!("ircd-oxide starting");
        self.ev.run(&mut self.top).expect("event loop stopped with error");
    }
}
