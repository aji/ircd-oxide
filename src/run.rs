// run.rs -- ircd-oxide runtime
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The runtime

use mio;
use std::io;
use std::io::prelude::*;
use std::rc::Rc;

use irc::client::ClientManager;
use state::world::WorldManager;

/// The top-level IRC server structure
pub struct IRCD<'obs> {
    clients: ClientManager,
    world: WorldManager<'obs>,
}

impl<'obs> IRCD<'obs> {
    /// Creates a new `IRCD`
    pub fn new() -> IRCD<'obs> {
        let clients = ClientManager::new();
        let mut world = WorldManager::new();

        world.observe(clients.clone());

        IRCD {
            clients: clients,
            world: world,
        }
    }
}

impl<'obs> mio::Handler for IRCD<'obs> {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        ev: &mut mio::EventLoop<IRCD>,
        token: mio::Token,
        events: mio::EventSet
    ) {
        // route the event as appropriate
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

    /// Runs the `Runner` forever
    pub fn run(&mut self) {
        info!("ircd-oxide starting");
        self.ev.run(&mut self.ircd);
    }
}
