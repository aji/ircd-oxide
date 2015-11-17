// irc/numeric.rs -- static IRC numerics database
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Read-only numerics database

// Technically correct is the best kind of correct?

/// A struct representing a particular IRC numeric.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Numeric(u32);

/// An invalid CAP subcommand was used
pub const ERR_INVALIDCAPCMD: Numeric = Numeric(410);

/// No nickname was provided
pub const ERR_NONICKNAMEGIVEN: Numeric = Numeric(431);

/// The requested nickname is not valid
pub const ERR_ERRONEOUSNICKNAME: Numeric = Numeric(432);

/// The requested nickname is in use
pub const ERR_NICKNAMEINUSE: Numeric = Numeric(433);

impl Numeric {
    /// Returns the integer value associated with this `Numeric`
    pub fn numeric(self) -> u32 { self.0 }

    /// Returns a C-style format string for the `Numeric` as represented on the
    /// wire for everything after the first space after the recipient's
    /// nickname. That is, if the line to send for the numeric was `:server.irc
    /// 432 user 123 :Invalid nickname`, then this function would return `"%s
    /// :Invalid nickname`". Only `%s` and `%%` are used, to simplify
    /// processing.
    pub fn string(self) -> &'static str {
        match self {
            ERR_INVALIDCAPCMD => "%s :Invalid CAP subcommand",
            ERR_NONICKNAMEGIVEN => ":No nickname given",
            ERR_ERRONEOUSNICKNAME => "%s :Invalid nickname",
            ERR_NICKNAMEINUSE => "%s :Nickname is already in use",

            // not sure what we should do here!
            _ => ":"
        }
    }
}
