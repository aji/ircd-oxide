// src/lib.rs -- the root of the `ircd` crate
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The library portion of ircd-oxide.
//!
//! ircd-oxide is structured as a large library whose main entry points are
//! assembled into a small `main` implementation in a separate binary.

//#![warn(missing_docs)]
//#![allow(unused_imports)]

extern crate bytes;
extern crate rand;
extern crate time;
extern crate tokio_core;
extern crate tokio_io;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

#[macro_use]
mod macros;

pub mod common;
pub mod crdb;
pub mod irc;
pub mod world;
