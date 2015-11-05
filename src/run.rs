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

use irc::client::ClientManager;
use irc::client::Listener;
use irc::client::PendingClient;
use irc::client::PendingClientAction;
use state::world::WorldManager;

/// The top-level IRC server structure
pub struct IRCD<'obs> {
    clients: ClientManager,
    tokens: HashMap<mio::Token, TokenData>,
    world: WorldManager<'obs>,
}

enum TokenData {
    Listener(Listener),
    PendingClient(PendingClient),
}

enum Action {
    Continue,
    ListenerAccept(PendingClient),
    PendingClient(PendingClientAction),
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

    fn add_listener(
        &mut self,
        listener: Listener,
        ev: &mut mio::EventLoop<IRCD>
    ) -> io::Result<()> {
        let token = mio::Token(random());
        try!(listener.register(token, ev));
        self.tokens.insert(token, TokenData::Listener(listener));
        Ok(())
    }

    fn add_pending_client(
        &mut self,
        pending: PendingClient,
        ev: &mut mio::EventLoop<IRCD>
    ) -> io::Result<()> {
        let token = mio::Token(random());
        try!(pending.register(token, ev));
        self.tokens.insert(token, TokenData::PendingClient(pending));
        Ok(())
    }
}

impl<'obs> mio::Handler for IRCD<'obs> {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        ev: &mut mio::EventLoop<IRCD>,
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

                    match listener.accept() {
                        Err(e) => {
                            error!("couldn't accept: {}", e);
                            Action::Continue
                        },

                        Ok(pending) => Action::ListenerAccept(pending),
                    }
                },

                TokenData::PendingClient(ref mut pending) => {
                    Action::PendingClient(pending.ready())
                },
            }
        };

        match action {
            Action::Continue => { },

            Action::ListenerAccept(pending) => {
                if let Err(e) = self.add_pending_client(pending, ev) {
                    error!("couldn't add pending client: {}", e);
                }
            },

            Action::PendingClient(pca) => match pca {
                PendingClientAction::Continue => {
                },

                PendingClientAction::Error |
                PendingClientAction::Close => {
                    match self.tokens.remove(&tk) {
                        Some(TokenData::PendingClient(pending)) => {
                            info!("dropping pending client");
                            if let Err(e) = pending.deregister(ev) {
                                error!("error when dropping pending \
                                        client: {}", e);
                            }
                        },
                        Some(tdata) => {
                            error!("logic error: bad token for \
                                    pending client close");
                            self.tokens.insert(tk, tdata);
                        },
                        None => {
                            error!("logic error: invalid token for \
                                    pending client close");
                        },
                    }
                },

                PendingClientAction::Promote => {
                },
            }
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

        self.ircd.add_listener(Listener::new(listener), &mut self.ev)
    }

    /// Runs the `Runner` forever
    pub fn run(&mut self) {
        info!("ircd-oxide starting");
        self.ev.run(&mut self.ircd).expect("event loop stopped with error");
    }
}
