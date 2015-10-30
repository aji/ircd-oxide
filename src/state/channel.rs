// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Channel state

use state::Clock;
use state::StateItem;

pub struct Channel {
    topic: Topic,
}

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
