// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use state::Channel;
use state::Id;

/// The top level struct that contains all conceptually global state.
#[derive(Clone)]
pub struct World {
    channels: HashMap<Id<Channel>, Channel>,
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> World {
        World {
            channels: HashMap::new(),
        }
    }

    /// Returns a reference to the channel map
    pub fn channels(&self) -> &HashMap<Id<Channel>, Channel> { &self.channels }

    /// Returns a mutable reference to the channel map
    pub fn channels_mut(&mut self) -> &mut HashMap<Id<Channel>, Channel> { &mut self.channels }
}
