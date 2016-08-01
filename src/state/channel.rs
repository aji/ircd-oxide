// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Channel state

use state::id::Id;
use state::identity::Identity;

/// An IRC channel.
pub struct Channel {
    id: Id<Channel>,
}

impl Channel {
    pub fn new(id: Id<Channel>) -> Channel {
        Channel { id: id }
    }
}

pub struct ChanUser;
