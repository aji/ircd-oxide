// state/user.rs -- user state management logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

use irc::IrcString;

/// The main user state object
///
/// Note that, in this IRC implementation, users are owned by the network! This
/// is contrary to typical IRC implementations where, although, users are
/// synchronized across all nodes, that data is only owned by the single server
/// the user is connected to. In ircd-oxide, we would like for a "user" to be
/// more abstract than an individual connection. And so, users are owned by the
/// whole network.
pub struct User {
    nick: Nickname,
}

pub struct Nickname {
    name: IrcString,
}
