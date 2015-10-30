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

pub struct World {
    channels: HashMap<Id<Channel>, Channel>,
    channel_names: ClaimMap<Channel, String>,
}
