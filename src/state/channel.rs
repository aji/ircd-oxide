// state/channel.rs -- channel state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Channel state

use common::bimap::{Bimap, AllA, AllB};
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

pub struct ChanUser {
    chan: Id<Channel>,
    user: Id<Identity>,
}

impl ChanUser {
    fn new(chan: Id<Channel>, user: Id<Identity>) -> ChanUser {
        ChanUser { chan: chan, user: user }
    }

    pub fn channel(&self) -> &Id<Channel> { &self.chan }

    pub fn user(&self) -> &Id<Identity> { &self.user }
}

pub struct ChanUserSet {
    set: Bimap<Id<Channel>, Id<Identity>, ChanUser>,
}

impl ChanUserSet {
    pub fn new() -> ChanUserSet {
        ChanUserSet { set: Bimap::new() }
    }

    pub fn join(&mut self, chan: Id<Channel>, user: Id<Identity>) -> &mut ChanUser {
        let cu = ChanUser::new(chan.clone(), user.clone());
        self.set.insert(chan, user, cu)
    }

    pub fn get(&mut self, chan: &Id<Channel>, user: &Id<Identity>) -> Option<&ChanUser> {
        self.set.get(chan, user)
    }

    pub fn get_mut(&mut self, chan: &Id<Channel>, user: &Id<Identity>) -> Option<&mut ChanUser> {
        self.set.get_mut(chan, user)
    }

    pub fn channels<'c>(&'c self, user: &Id<Identity>) -> AllA<'c, ChanUser> {
        self.set.all_a(user)
    }

    pub fn members<'c>(&'c self, chan: &Id<Channel>) -> AllB<'c, ChanUser> {
        self.set.all_b(chan)
    }
}
