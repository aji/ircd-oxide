// src/lib.rs -- the root of the `ircd` crate
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The library portion of ircd-oxide.
//!
//! ircd-oxide is structured as a large library whose main entry points are
//! assembled into a small `main` implementation in a separate binary.

#![warn(missing_docs)]

extern crate mio;
extern crate rand;
extern crate time;

#[macro_use]
extern crate log;

pub mod irc;
pub mod oxen;
pub mod run;
pub mod state;
pub mod tcp;
pub mod util;
pub mod xenc;
