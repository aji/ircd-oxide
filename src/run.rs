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

use irc::listen::Listener;
use irc::pending::PendingClient;
use state::world::WorldManager;

/// The top-level IRC server structure
pub struct Top {
    tokens: HashMap<mio::Token, TokenData>,
}

enum TokenData {
    Listener(Listener),
}

enum Action {
    Continue,
}

impl Top {
    /// Creates a new `Top`
    pub fn new() -> Top {
        Top {
            tokens: HashMap::new(),
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
                    // TODO accept
                },
            }

            Action::Continue
        };

        match action {
            Action::Continue => { },
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
