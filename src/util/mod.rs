// util/mod.rs -- various generic utilities
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Generic structures, or types owned by no single subsystem.

pub mod sid;
pub mod table;

pub use self::sid::Sid;
pub use self::table::Table;
