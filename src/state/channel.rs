// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Channel state

use std::collections::HashMap;

use state::Clock;
use state::Clocked;
use state::Id;
use state::StateItem;

/// An IRC channel.
#[derive(Clone)]
pub struct Channel {
    topic: Topic,
    users: HashMap<Id<()>, ChannelUser>,
}

/// Extra data associated with a user in an IRC channel.
#[derive(Clone)]
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

/// An IRC channel topic (may later be merged into `Channel` as a
/// `Clocked<String>`).
#[derive(Clone)]
pub struct Topic {
    ts: Clock,
    text: String,
}

impl StateItem for Topic {
    fn merge(&mut self, other: &Topic) -> &mut Topic {
        if self.ts < other.ts {
            self.ts    = other.ts.clone();
            self.text  = other.text.clone();
        }

        self
    }
}
