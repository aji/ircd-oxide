// util/table.rs -- a generic table type
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! A table type, essentially a two-dimensional hash map.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;

/// Conceptually, a function *t* : *K* &times; *K* &rarr; *V*
pub struct Table<K, V> {
    rows: HashMap<K, HashMap<K, V>>,
}

impl<K, V> Table<K, V> where K: Hash + Eq {
    /// Creates an empty table.
    pub fn new() -> Table<K, V> {
        Table { rows: HashMap::new() }
    }

    /// Fetches a reference to the value for the given row and column, if it
    /// exists.
    pub fn get(&self, k1: &K, k2: &K) -> Option<&V> {
        self.rows.get(k1).and_then(|r| r.get(k2))
    }

    /// Fetches a mutable reference to the value for the given row and column,
    /// if it exists.
    pub fn get_mut(&mut self, k1: &K, k2: &K) -> Option<&mut V> {
        self.rows.get_mut(k1).and_then(|r| r.get_mut(k2))
    }

    /// Inserts a value at the given row and column, replacing any previous
    /// value that may have been there.
    pub fn put(&mut self, k1: K, k2: K, v: V) {
        self.rows.entry(k1).or_insert_with(|| HashMap::new()).insert(k2, v);
    }

    /// Piggybacking on the `entry` API in the standard `HashMap` collection.
    pub fn entry(&mut self, k1: K, k2: K) -> Entry<K, V> {
        self.rows.entry(k1).or_insert_with(|| HashMap::new()).entry(k2)
    }
}
