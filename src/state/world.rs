// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use state::Identity;
use state::Channel;
use state::Id;

/// The top level struct that contains all conceptually global state.
#[derive(Clone)]
pub struct World {
    counter: usize,
    identities: HashMap<Id<Identity>, Identity>,
    channels: HashMap<Id<Channel>, Channel>,
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> World {
        World {
            counter: 1,
            identities: HashMap::new(),
            channels: HashMap::new(),
        }
    }

    /// Returns a reference to the counter
    pub fn counter(&self) -> &usize { &self.counter }

    /// Returns a mutable reference to the counter
    pub fn counter_mut(&mut self) -> &mut usize { &mut self.counter }

    /// Returns a mutable reference to the identity map
    pub fn identities(&self) -> &HashMap<Id<Identity>, Identity> {
        &self.identities
    }

    /// Returns a mutable reference to the identity map
    pub fn identities_mut(&mut self) -> &mut HashMap<Id<Identity>, Identity> {
        &mut self.identities
    }

    /// Returns a reference to the channel map
    pub fn channels(&self) -> &HashMap<Id<Channel>, Channel> { &self.channels }

    /// Returns a mutable reference to the channel map
    pub fn channels_mut(&mut self) -> &mut HashMap<Id<Channel>, Channel> { &mut self.channels }
}
