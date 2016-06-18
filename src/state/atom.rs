// state/atom.rs -- Atoms of global state
// Copyright (C) 2016 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Atoms of global state

use state;
use state::Id;

#[derive(PartialEq, Eq)]
pub enum AtomId {
    Identity(Id<state::Identity>),
}

pub enum Atom {
    Identity(state::Identity),
}

impl Atom {
    pub fn id(&self) -> AtomId {
        match self {
            &Atom::Identity(ref x) => AtomId::Identity(x.id().clone())
        }
    }
}
