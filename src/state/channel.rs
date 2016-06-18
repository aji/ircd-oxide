// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Channel state

this file is not to be compiled at present

use std::collections::HashMap;

use state::Clocked;
use state::Id;
use state::StateItem;

/// An IRC channel.
#[derive(Clone, PartialEq, Eq)]
pub struct Channel {
    /// The channel topic, as a timestamped `String`
    pub topic: Clocked<String>,
    users: HashMap<Id<()>, ChannelUser>,
}

/// Extra data associated with a user in an IRC channel.
#[derive(Clone, PartialEq, Eq)]
pub struct ChannelUser {
    is_chanop: Clocked<bool>,
    is_voiced: Clocked<bool>,
}

impl StateItem for ChannelUser {
    fn merge(&mut self, other: &ChannelUser) -> &mut ChannelUser {
        self.is_chanop.merge(&other.is_chanop);
        self.is_voiced.merge(&other.is_voiced);

        self
    }
}
