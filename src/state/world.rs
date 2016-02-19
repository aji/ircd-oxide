// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use state::Channel;
use state::Id;
use state::Identity;
use state::IdentitySet;

/// The top level struct that contains all conceptually global state.
#[derive(Clone)]
pub struct World {
    identities: IdentitySet,
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> World {
        World {
            identities: IdentitySet::new(),
        }
    }

    /// Forwards to `IdentitySet::create_temp_identity`
    pub fn create_temp_identity(&mut self, id: Id<Identity>) {
        self.identities.create_temp_identity(id)
    }
}
