// run.rs -- ircd-oxide runtime
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The runtime

use irc::global::IRCD;
use looper::LooperActions;
use state::checkpoint::Change;
use state::world::World;
use state::world::WorldGuard;

/// The top-level IRC server structure
pub struct Context {
    pub ircd: IRCD,
    world: World
}

pub type Message = ();

pub struct Guard<'a> {
    pub ircd: &'a IRCD,
    pub world: WorldGuard<'a>,
}

impl Context {
    /// Creates a new `Top`
    pub fn new() -> Context {
        let ircd = IRCD::new();
        let world = World::new(ircd.sid().clone());

        Context {
            ircd: ircd,
            world: world,
        }
    }

    pub fn on_event<F>(&mut self, act: &mut LooperActions, cb: F)
    where F: FnOnce(&mut Guard, &mut LooperActions) {
        let mut guard = Guard {
            ircd: &self.ircd,
            world: self.world.editor(),
        };

        cb(&mut guard, act);

        let changes = guard.finish();
        info!("there were {} changes", changes.len());
    }
}

impl<'a> Guard<'a> {
    fn finish(self) -> Vec<Change> {
        self.world.finish()
    }
}
