// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! Channel state

use state::Clock;
use state::StateItem;

pub struct Channel {
    topic: Topic,
}

pub struct Topic {
    ts: Clock,
    text: String,
}

impl StateItem for Channel {
    fn identity() -> Channel {
        Channel {
            topic: StateItem::identity(),
        }
    }

    fn merge(&mut self, other: &Channel) -> &mut Channel {
        self.topic.merge(&other.topic);

        self
    }
}

impl StateItem for Topic {
    fn identity() -> Topic {
        Topic {
            ts: StateItem::identity(),
            text: String::new(),
        }
    }

    fn merge(&mut self, other: &Topic) -> &mut Topic {
        if self.ts < other.ts {
            self.ts    = other.ts.clone();
            self.text  = other.text.clone();
        }

        self
    }
}
