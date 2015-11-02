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
use std::mem;
use std::net::ToSocketAddrs;

use irc::client::ClientManager;
use irc::client::Listener;
use state::world::WorldManager;

/// The top-level IRC server structure
pub struct IRCD<'obs> {
    clients: ClientManager,
    tokens: HashMap<mio::Token, TokenData>,
    world: WorldManager<'obs>,
}

enum TokenData {
    Listener(Listener),
    Client,
}

impl<'obs> IRCD<'obs> {
    /// Creates a new `IRCD`
    pub fn new() -> IRCD<'obs> {
        let clients = ClientManager::new();
        let mut world = WorldManager::new();

        world.observe(clients.clone());

        IRCD {
            clients: clients,
            tokens: HashMap::new(),
            world: world,
        }
    }

    fn generate_token(&self) -> mio::Token {
        mio::Token(random())
    }

    fn add_listener(&mut self, tk: mio::Token, listener: mio::tcp::TcpListener) {
        self.tokens.insert(tk, TokenData::Listener(Listener::new(listener)));
    }
}

impl<'obs> mio::Handler for IRCD<'obs> {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        ev: &mut mio::EventLoop<IRCD>,
        tk: mio::Token,
        events: mio::EventSet
    ) {
        debug!("event becomes ready");

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
                listener.accept();
            },

            TokenData::Client => {
            },
        }
    }
}

/// A structure for running an `IRCD`
pub struct Runner<'obs> {
    ircd: IRCD<'obs>,
    ev: mio::EventLoop<IRCD<'obs>>,
}

impl<'obs> Runner<'obs> {
    /// Creates a new `Runner`
    pub fn new() -> io::Result<Runner<'obs>> {
        Ok(Runner {
            ircd: IRCD::new(),
            ev: try!(mio::EventLoop::new()),
        })
    }

    /// Gets a reference to the `IRCD`
    pub fn ircd(&'obs mut self) -> &mut IRCD {
        &mut self.ircd
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

        let token = self.ircd.generate_token();

        try!(self.ev.register_opt(
            &listener,
            token,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        ));

        self.ircd.add_listener(token, listener);

        Ok(())
    }

    /// Runs the `Runner` forever
    pub fn run(&mut self) {
        info!("ircd-oxide starting");
        self.ev.run(&mut self.ircd);
    }
}
