// state/checkpoint.rs -- state checkpointing
// Copyright (C) 2016 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! State checkpointing

use state::atom::Atom;
use state::atom::AtomId;
use state::atom::Atomic;

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

    pub fn added<A: Atomic>(&mut self, added: &A) {
        self.changes.push(Change::Add(added.atom_id()));
    }

    pub fn finish(self) -> Vec<Change> {
        self.changes
    }
}
