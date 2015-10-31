// irc/client.rs -- client protocol handlers
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client protocol handlers

use std::collections::HashMap;

use state::world;
use state::Channel;
use state::Diffable;
use state::Differ;
use state::Id;
use state::World;

/// A pool of clients
pub struct ClientPool;

/// The structure that holds a pool of clients and responds to events
pub struct ClientManager {
    pool: ClientPool
}

impl ClientManager {
    /// Creates a new `ClientManager` with an empty `ClientPool`
    pub fn new() -> ClientManager {
        ClientManager {
            pool: ClientPool
        }
    }

    fn channels_changed(
        &mut self,
        old: &HashMap<Id<Channel>, Channel>,
        new: &HashMap<Id<Channel>, Channel>
    ) {
        for diff in old.diff(new) {
            match diff {
                Differ::Added((id, chan)) => {
                    self.channel_added(id, chan);
                },

                Differ::Removed((id, chan)) => {
                    self.channel_removed(id, chan);
                },

                Differ::Changed((id, chan_old), (_, chan_new)) => {
                    self.channel_changed(id, chan_old, chan_new);
                },
            }
        }
    }

    fn channel_added(&mut self, id: &Id<Channel>, chan: &Channel) {
        println!("channel added");
    }

    fn channel_removed(&mut self, id: &Id<Channel>, chan: &Channel) {
        println!("channel added");
    }

    fn channel_changed(
        &mut self,
        id: &Id<Channel>,
        chan_old: &Channel,
        chan_new: &Channel
    ) {
        println!("channel changed");
    }
}

impl world::Observer for ClientManager {
    fn world_changed(&mut self, old: &World, new: &World) {
        self.channels_changed(old.channels(), new.channels());
    }
}
