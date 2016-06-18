// state/nickname.rs -- nickname handling
// Copyright (C) 2016 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Nickname handling

use std::collections::HashMap;

use state::claim::Claim;
use state::id::Id;
use state::identity::Identity;

pub struct NicknameMap {
    nicks: HashMap<u64, Nickname>,
    next_nick: u64,
    nick_by_owner: HashMap<Id<Identity>, u64>,
    nick_by_name: HashMap<String, u64>,
}

struct Nickname {
    claim: Claim<Identity, Nickname>,
    name: String,
}

impl NicknameMap {
    pub fn new() -> NicknameMap {
        NicknameMap {
            nicks: HashMap::new(),
            next_nick: 0,
            nick_by_owner: HashMap::new(),
            nick_by_name: HashMap::new(),
        }
    }

    pub fn nickname(&self, owner: &Id<Identity>) -> Option<&String> {
        self.nick_by_owner
            .get(owner)
            .and_then(|i| self.nicks.get(i))
            .map(|nn| &nn.name)
    }

    pub fn owner(&self, name: &String) -> Option<&Id<Identity>> {
        self.nick_by_name
            .get(name)
            .and_then(|i| self.nicks.get(i))
            .and_then(|nn| nn.claim.owner())
    }
}
