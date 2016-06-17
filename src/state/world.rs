// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use common::Sid;
use state::Channel;
use state::Id;
use state::IdGenerator;
use state::Identity;
use state::IdentitySet;

/// A trait that defines operations a world-changer can perform
pub trait WorldView {
    /// Creates a temporary identity and returns its ID
    fn create_temp_identity(&mut self) -> Id<Identity>;
}

/// The top level struct that contains all conceptually global state.
pub struct World {
    // strictly global:
    identities: IdentitySet,

    // strictly local:
    sid: Sid,
    idgen_identity: IdGenerator<Identity>,
}

impl World {
    /// Creates an empty `World` with the given server ID
    pub fn new(sid: Sid) -> World {
        World {
            sid: sid.clone(),
            identities: IdentitySet::new(),
            idgen_identity: IdGenerator::new(sid.clone()),
        }
    }

    // Returns a reference to the world that can be used to make changes.
    pub fn editor(&mut self) -> WorldGuard {
        WorldGuard { world: self }
    }
}

/// A struct for making changes to a World. Changes are tracked
pub struct WorldGuard<'w> {
    world: &'w mut World,
}

impl<'w> WorldView for WorldGuard<'w> {
    fn create_temp_identity(&mut self) -> Id<Identity> {
        let id = self.world.idgen_identity.next();
        self.world.identities.create_temp_identity(id.clone());
        id
    }
}
