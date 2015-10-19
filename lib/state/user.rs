// state/user.rs -- user state management logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! User state management logic

use irc::IrcString;
use state::Claim;
use state::Id;

/// A user's identity
pub struct Identity;

/// A nickname
pub struct Nickname {
    id: Id<Nickname>,
    text: IrcString,
    claim: Claim<Identity, Nickname>
}

impl Nickname {
    /// Get the globally unique ID for this nickname.
    pub fn id(&self) -> &Id<Nickname> { &self.id }

    /// Get the text of this nickname.
    pub fn text(&self) -> &IrcString { &self.text }

    /// Get the claim to this nickname.
    pub fn claim(&self) -> &Claim<Identity, Nickname> { &self.claim }
}
