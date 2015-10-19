// state/masklist.rs -- mask/ban list handling
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Mask/ban lists

use std::collections::HashMap;

use state::Clock;
use state::StateItem;

/// A list of masks
#[derive(Clone)]
pub struct MaskList {
    masks: HashMap<String, MaskListEntry>,
}

/// An entry in the list of masks
#[derive(Clone)]
pub struct MaskListEntry {
    added: Clock,
    removed: Option<Clock>,
}

impl StateItem for MaskList {
    fn merge(&mut self, other: &MaskList) -> &mut MaskList {
        for (mask, other_ent) in other.masks.iter() {
            self.masks.entry(mask.clone())
                    .or_insert_with(|| MaskListEntry::empty())
                    .merge(other_ent);
        }

        self
    }
}

impl MaskListEntry {
    fn empty() -> MaskListEntry {
        MaskListEntry {
            added: Clock::neg_infty(),
            removed: None
        }
    }
}

impl StateItem for MaskListEntry {
    fn merge(&mut self, other: &MaskListEntry) -> &mut MaskListEntry{
        // we have to always merge the newly added clock. failure to do so
        // could result in add times diverging.
        self.added.merge(&other.added);

        if let Some(other_removed) = other.removed {
            self.removed = self.removed
                    .or_else(|| Some(Clock::neg_infty()))
                    .as_mut().map(|r| *r.merge(&other_removed));
        }

        self
    }
}
