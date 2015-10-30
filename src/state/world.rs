// state/world.rs -- top level state object
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The top level state object

use std::collections::HashMap;

use state::Channel;
use state::ClaimMap;
use state::Id;

/// The top level struct that contains all conceptually global state.
#[derive(Clone)]
pub struct World {
    channels: HashMap<Id<Channel>, Channel>,
    channel_names: ClaimMap<Channel, String>,
}

impl World {
    pub fn new() -> World {
        World {
            channels: HashMap::new(),
            channel_names: ClaimMap::new(),
        }
    }
}

/// A trait for objects that can observe changes to the world and act on them.
pub trait Observer {
    fn world_changed(&mut self, old: &World, new: &World);
}

/// A struct for managing a [`World`](struct.World.html).
pub struct WorldManager {
    world: World,
    observers: Vec<Box<Observer>>,
}

impl WorldManager {
    pub fn new() -> WorldManager {
        WorldManager {
            world: World::new(),
            observers: Vec::new(),
        }
    }
}
