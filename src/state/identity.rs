// state/identity.rs -- user identities
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! User identities
//!
//! Identities are the primary "owners" of IRC state. For example, a nickname
//! has at most one identity that owns it at any given point in time. Similarly,
//! channel members are identified by an identity.
//!
//! Although the state layer is not concerned with mapping identities to
//! connections, certain details of this mapping leak in to the implementation.
//! The most notable is the concept of a temporary identity. Temporary
//! identities are connection-scoped, meaning they are uniquely established by a
//! connection, and dropped together with the connection that created them. At
//! any point, a user with a temporary identity may assume a non-temporary
//! identity, whether by registration, identification, asynchronous methods,
//! etc.

use std::collections::HashMap;

use state::Id;

/// A user identity.
#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    id: Id<Identity>,
    temporary: bool
}

impl Identity {
    /// Creates a new `Identity` with the given `Id`. The `temp` flag indicates
    /// whether the `Identity` is temporary (connection-scoped) or not.
    pub fn new(id: Id<Identity>, temp: bool) -> Identity {
        Identity { id: id, temporary: temp }
    }

    /// Returns a reference to the `Id`
    pub fn id(&self) -> &Id<Identity> {
        &self.id
    }

    /// Returns whether the `Identity` is temporary or not.
    pub fn temporary(&self) -> bool {
        self.temporary
    }
}

/// A set of identities
#[derive(Clone)]
pub struct IdentitySet {
    identities: HashMap<Id<Identity>, Identity>,
}

impl IdentitySet {
    /// Creates an empty identity set
    pub fn new() -> IdentitySet {
        IdentitySet {
            identities: HashMap::new(),
        }
    }

    /// Gets an `Identity` given its `Id`.
    pub fn get(&self, id: &Id<Identity>) -> Option<&Identity> {
        self.identities.get(id)
    }

    /// Creates a new temporary `Identity`
    pub fn create_temp_identity(&mut self, id: Id<Identity>) {
        let identity = Identity::new(id.clone(), true);
        self.identities.insert(id, identity);
    }
}
