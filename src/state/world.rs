// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use common::Sid;
use state::atom::Atomic;
use state::checkpoint::Changes;
use state::checkpoint::Change;
use state::claim::ClaimSet;
use state::id::Id;
use state::id::IdGenerator;
use state::id::IdMap;
use state::identity::Identity;

/// A trait that defines operations a world-changer can perform
pub trait WorldView {
    // MUTATIONS
    // ====================

    /// Creates a temporary identity and returns its ID
    fn create_temp_identity(&mut self) -> Id<Identity>;

    /// Claims a nickname for an identity. Returns whether the claim was successful.
    fn nick_claim(&mut self, owner: Id<Identity>, nick: String) -> bool;

    /// Changes an identity's active nickname. Returns whether the operation was successful.
    fn nick_use(&mut self, owner: Id<Identity>, nick: String) -> bool;

    // READ-ONLY
    // ====================

    fn nickname(&self, owner: &Id<Identity>) -> Option<&String>;
}

/// The top level struct that contains all conceptually global state.
pub struct World {
    // strictly global:
    identities: IdMap<Identity>,
    nicknames: NicknameMap,

    // strictly local:
    sid: Sid,
    idgen_identity: IdGenerator<Identity>,
}

impl World {
    /// Creates an empty `World` with the given server ID
    pub fn new(sid: Sid) -> World {
        World {
            identities: IdMap::new(),
            nicknames: NicknameMap::new(sid),

            sid: sid,
            idgen_identity: IdGenerator::new(sid),
        }
    }

    /// Returns a reference to the world that can be used to make changes.
    pub fn editor<'w>(&'w mut self) -> WorldGuard<'w> {
        WorldGuard::new(self)
    }
}

/// A struct for handling mappings from nicknames to users
struct NicknameMap {
    nicknames: ClaimSet<Identity, Nickname>
}

/// A nickname
#[derive(Clone, Hash, PartialEq, Eq)]
struct Nickname(String);

impl NicknameMap {
    fn new(sid: Sid) -> NicknameMap {
        NicknameMap { nicknames: ClaimSet::new(sid) }
    }
}

/// A struct for making changes to a World. Changes are tracked
pub struct WorldGuard<'w> {
    changes: Changes,
    world: &'w mut World,
}

impl<'w> WorldGuard<'w> {
    fn new<'v>(world: &'v mut World) -> WorldGuard<'v> {
        WorldGuard {
            changes: Changes::new(),
            world: world
        }
    }

    pub fn finish(self) -> Vec<Change> {
        self.changes.finish()
    }
}

impl<'w> WorldView for WorldGuard<'w> {
    fn create_temp_identity(&mut self) -> Id<Identity> {
        let id = self.world.idgen_identity.next();
        let identity = Identity::new(id.clone(), true);
        self.changes.add(Change::Add(identity.atom_id()));
        self.world.identities.insert(id.clone(), identity);
        id
    }

    fn nick_claim(&mut self, owner: Id<Identity>, nick: String) -> bool {
        self.world.nicknames.nicknames.claim(owner, Nickname(nick))
    }

    fn nick_use(&mut self, owner: Id<Identity>, nick: String) -> bool {
        self.world.nicknames.nicknames.set_active(owner, Nickname(nick))
    }

    fn nickname(&self, owner: &Id<Identity>) -> Option<&String> {
        self.world.nicknames.nicknames.active(owner).map(|n| &n.0)
    }
}
