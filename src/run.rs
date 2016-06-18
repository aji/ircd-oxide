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
use state::checkpoint::Change;
use state::world::World;

/// The top-level IRC server structure
pub struct Top {
    ircd: IRCD,
    world: World,
    tokens: HashMap<mio::Token, TokenData>,
    ch: ClientHandler,
}

enum TokenData {
    Listener(Listener),
    Client(Client),
}

/// An action to be performed by the run loop after handling an event.
pub enum Action {
    /// Do nothing
    Continue,
    /// Drop the peer that handled the event
    DropPeer,
    /// Add a client
    AddClient(Client),
}

impl Top {
    /// Creates a new `Top`
    pub fn new() -> Top {
        let ircd = IRCD::new();
        let world = World::new(ircd.sid.clone());

        Top {
            ircd: ircd,
            world: world,
            tokens: HashMap::new(),
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

    fn add_client(
        &mut self,
        client: Client,
        ev: &mut mio::EventLoop<Top>
    ) -> io::Result<()> {
        let token = mio::Token(random());
        try!(client.register(token, ev));
        self.tokens.insert(token, TokenData::Client(client));
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

        let mut changes: Option<Vec<Change>> = None;

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
                        Ok(client) => Action::AddClient(client),

                        Err(e) => {
                            error!("accepting client: {}", e);
                            Action::Continue
                        }
                    }
                },

                TokenData::Client(ref mut client) => {
                    let mut editor = self.world.editor();

                    let act = match client.ready(&self.ircd, &mut editor, &self.ch) {
                        Ok(action) => action,

                        Err(e) => {
                            info!("dropping client: {}", e);
                            Action::DropPeer
                        }
                    };

                    changes = Some(editor.finish());

                    act
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

            Action::AddClient(client) => {
                if let Err(e) = self.add_client(client, ev) {
                    error!("error adding client: {}", e);
                }
            },
        }

        if let Some(changes) = changes {
            info!("there were {} changes", changes.len());
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
            debug!("listening on {:?}", addr);
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
