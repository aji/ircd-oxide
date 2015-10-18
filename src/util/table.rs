// util/table.rs -- a generic table type
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::hash::Hash;

/// Conceptually, a function *t* : *K* &times; *K* &rarr; *V*
pub struct Table<K, V> {
    rows: HashMap<K, HashMap<K, V>>,
}

impl<K, V> Table<K, V> where K: Hash + Eq {
    pub fn new() -> Table<K, V> {
        Table { rows: HashMap::new() }
    }

    pub fn get(&mut self, k1: &K, k2: &K) -> Option<&V> {
        self.rows.get(k1).and_then(|r| r.get(k2))
    }

    pub fn put(&mut self, k1: K, k2: K, v: V) {
        self.rows.entry(k1).or_insert_with(|| HashMap::new()).insert(k2, v);
    }
}
