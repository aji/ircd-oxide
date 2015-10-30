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
    /// Creates an empty `World`.
    pub fn new() -> World {
        World {
            channels: HashMap::new(),
            channel_names: ClaimMap::new(),
        }
    }
}

/// A trait for objects that can observe changes to the world and act on them.
pub trait Observer {
    /// Called when the `World` has changed. The caller is free to examine the
    /// old and new `World`s to determine how to act.
    fn world_changed(&mut self, old: &World, new: &World);
}

/// A struct for managing a [`World`](struct.World.html).
pub struct WorldManager<'obs> {
    world: World,
    observers: Vec<Box<Observer + 'obs>>,
}

impl<'obs> WorldManager<'obs> {
    /// Creates a `WorldManager` with an empty `World`. In your new `World` you
    /// can be a heavy-handed dictator, a benevolent monarch, or establish a
    /// socialist oligarchy. The choice is yours!
    pub fn new() -> WorldManager<'obs> {
        WorldManager {
            world: World::new(),
            observers: Vec::new(),
        }
    }

    /// Adds an `Observer` to the list of observers of this world
    pub fn observe<O: Observer + 'obs>(&mut self, obs: O) {
        self.observers.push(Box::new(obs));
    }

    /// Calls a function to close the given channel, if it exists.
    pub fn update_channel<F>(&mut self, chanid: Id<Channel>, cb: F)
    where F: FnOnce(&mut Channel) {
        let old = self.world.clone();

        if let Some(chan) = self.world.channels.get_mut(&chanid) {
            cb(chan);
        }

        self.notify_observers(old);
    }

    fn notify_observers(&mut self, old: World) {
        for obs in self.observers.iter_mut() {
            obs.world_changed(&old, &self.world);
        }
    }
}
