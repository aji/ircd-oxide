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

/// A trait that defines operations a world-changer can perform
pub trait WorldView {
    /// Creates a temporary identity with the given ID
    fn create_temp_identity(&mut self, id: Id<Identity>);
}

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
}

impl WorldView for World {
    fn create_temp_identity(&mut self, id: Id<Identity>) {
        self.identities.create_temp_identity(id)
    }
}
