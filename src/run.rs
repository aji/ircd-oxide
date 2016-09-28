// run.rs -- ircd-oxide runtime
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The runtime

use irc::global::IRCD;
use looper::Context;
use looper::LooperActions;
use state::world::World;
use state::world::WorldGuard;

/// The top-level IRC server structure
pub struct Top {
    pub ircd: IRCD,
    world: World,
}

impl Top {
    /// Creates a new `Top`
    pub fn new() -> Top {
        let ircd = IRCD::new();
        let world = World::new(ircd.sid().clone());

        Top {
            ircd: ircd,
            world: world,
        }
    }

    pub fn edit<'w, 't: 'w, F, T>(&'t mut self, f: F) -> T
    where F: Fn(&mut WorldGuard<'w>) -> T {
        let mut guard = self.world.editor();
        let result = f(&mut guard);
        let changes = guard.finish();
        info!("there were {} changes", changes.len());
        result
    }
}

impl Context for Top {
    type Message = ();

    fn on_event<F>(&mut self, act: &mut LooperActions<Top>, cb: F)
    where F: FnOnce(&mut Top, &mut LooperActions<Top>) {
        info!("event will be handled");
        cb(self, act);
        info!("event has been handled");
    }
}
