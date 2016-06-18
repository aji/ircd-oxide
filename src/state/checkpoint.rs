// state/checkpoint.rs -- state checkpointing
// Copyright (C) 2016 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! State checkpointing

use state::atom::Atom;
use state::atom::AtomId;

pub struct Changes {
    changes: Vec<Change>
}

pub enum Change {
    Add(AtomId),
    Delete(Atom),
    Update(Atom, AtomId),
}

impl Changes {
    pub fn new() -> Changes {
        Changes { changes: Vec::new() }
    }

    pub fn add(&mut self, change: Change) {
        self.changes.push(change);
    }

    pub fn finish(self) -> Vec<Change> {
        self.changes
    }
}
