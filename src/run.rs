// run.rs -- ircd-oxide runtime
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The runtime

use mio;

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
        IRCD {
            clients: ClientManager::new(),
            world: WorldManager::new(),
        }
    }

    pub fn run(&'obs mut self, ev: &mut mio::EventLoop<IRCD>) {
        self.world.observe(&mut self.clients);
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
