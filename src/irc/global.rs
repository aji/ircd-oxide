// irc/global.rs -- global types
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! "Global" types.
//!
//! Although we don't store these as global variables, in the traditional sense,
//! they're structures that many parts of the IRC handling infrastructure need
//! access to, and so we pass them around.

/// The top level IRC server type
pub struct IRCD;

impl IRCD {
    /// Creates a new `IRCD`
    pub fn new() -> IRCD {
        IRCD
    }
}
