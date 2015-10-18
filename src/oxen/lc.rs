// oxen/lc.rs -- last contact
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use time;

use util::Table;

pub struct LastContact {
    tab: Table<Sid, time::Timespec>,
}
