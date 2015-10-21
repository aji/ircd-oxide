// src/lib.rs -- the root of the `ircd` crate
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

extern crate time;

#[macro_use]
extern crate log;

pub mod irc;
pub mod oxen;
pub mod state;
pub mod util;
pub mod xenc;
