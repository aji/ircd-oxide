//! In-memory database built on a hierarchy of CRDTs
//!
//! A CRDB instance is a collection of named **tables**. Tables are little more than key-value
//! mappings, where keys are strings. The structure of the rows in the table is determined by that
//! table's **schema**. The schema determines how items are converted to and from bytes, and how
//! items are merged together. The merge operation is essential to guaranteeing eventually
//! consistent convergence across replicas, and is described later.
//!
//! # Transactions
//!
//! All updates to a CRDB are done within the context of a transaction. This term is used loosely,
//! and really only refers to a batch of updates that can be rolled back.
//!
//! Two kinds of transaction are available:
//!
//!  * CRDB-level transactions. These can only be created from an existing batch of new records.
//!    These are most useful for tasks that don't care about the contents of the records, like
//!    loading a database from disk, or applying updates received from another replica.
//!
//!  * Table-level transactions. These use the decoded form of the data in the table, determined
//!    by that table's schema. Table-level transactions still need to be committed on the CRDB
//!    instance that contains the table.
//!
//! When a transaction is committed, the changes are applied to the underlying tables, and updates
//! are broadcast to observers.
//!
//! # Observing updates
//!
//! CRDB builds on the primitives in the `common::observe` module for distributing information
//! about how the database is changing. Similarly to transactions, two kinds of observer are
//! available:
//!
//!  * CRDB-level observers, which see all updates to all tables. These are "raw" updates, meaning
//!    the update only contains the encoded `Record` that was changed. This kind of observer is
//!    most useful for implementing persistence or replication, where the exact contents of the
//!    records is not important.
//!
//!  * Table-level observers, which see all updates to that table. These are "typed" updates,
//!    meaning they contain the decoded form of a record, determined by the schema.
//!
//! Observation "completion" is tied to transaction completion. Blocking completion of an
//! observation also blocks the transaction that produced the update.
//!
//! # Merging items
//!
//! At the core of CRDB is the idea of a "merge" operation, which has certain invariants.
//! Abstractly, a "merge" is a function that takes two rows and merges them into a new row:
//! `merge : (R, R) -> R`, where `R` is the set of possible rows. In order to ensure
//! that a table remains consistent, its schema's merge operation must guarantee the following
//! properties:
//!
//!  * **Idempotency**: `merge(a, a) = a`. That
//!
//!  * **Commutative**: `merge(a, b) = merge(b, a)`. That is, the
//!    "direction" of the merge does not matter.
//!
//!  * **Associative**: `merge(a, merge(b, c)) = `merge(merge(a, b), c)`. That is, merging
//!    three or more items can be done my merging them in pairs, in no particular order.
//!
//! Simple examples of schemas that meet these criteria:
//!
//!  * An integer, where two integers are "merged" by taking the maximum.
//!
//!  * A set, where two sets are merged by taking the union.
//!
//! Simple examples of schemas that do *not* meet these critera:
//!
//!  * An integer, where two integers are "merged" by adding them. (This is not idempotent.)
//!
//!  * A set, where two sets are "merged" by only taking elements unique to the second set.
//!    (This is neither idempotent, commutative, nor associative.)
//!
//! Refer to the [Wikipedia article about CRDTs][wiki] for more information about this idea.
//!
//! [wiki]: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type

use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::collections::hash_map::Values;
use std::fmt;
use std::hash::Hash;
use std::rc::Rc;
use std::vec;

use futures::Async;
use futures::Future;
use futures::Poll;

use rand::random;

use common::observe;
use common::observe::Observable;
use common::observe::Observer;

#[cfg(test)]
mod tests;

/// A record, which is just a vector of bytes.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Record(pub Vec<u8>);

impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Record({:?})", self.0)
    }
}

/// An eventually consistent database. See module-level documentation for more information.
pub struct CRDB {
    updates: Observable<RawUpdates>,
    tables: HashMap<String, Box<RawTable>>,
}

impl CRDB {
    /// Creates an empty CRDB
    pub fn new() -> CRDB {
        CRDB {
            updates: Observable::new(),
            tables: HashMap::new(),
        }
    }

    /// Creates a table using the given schema.
    ///
    /// # Panics
    ///
    /// This method will panic if the named table is already in use.
    pub fn create_table<S: 'static + Schema>(&mut self, name: &str, schema: S) -> Table<S> {
        let inner = {
            let inner = TableInner {
                name: name.to_string(),
                schema: schema,
                rows: HashMap::new(),
                updates: Observable::new(),
            };
            Rc::new(RefCell::new(inner))
        };

        let raw = Table { inner: inner.clone() };
        let typed = Table { inner: inner };

        let prev = self.tables.insert(name.to_string(), Box::new(raw));

        if prev.is_some() {
            panic!("table name reused");
        }

        typed
    }

    /// Returns an `Observer` for the stream of raw updates across all tables
    pub fn updates(&mut self) -> Observer<RawUpdates> {
        self.updates.observer()
    }

    /// Commits a raw transaction
    pub fn commit_raw(&mut self, tx: RawTransaction) -> Completion {
        let mut completions = Vec::new();
        let mut updates = Vec::new();
        let txid = tx.txid;

        for (table_name, items) in tx.items.into_iter() {
            let mut table = match self.tables.get_mut(&table_name) {
                Some(table) => table,
                None => {
                    warn!("discarding commit of {} items to {}", items.len(), table_name);
                    continue;
                }
            };

            completions.push(table.commit_all_raw(txid, items, &mut updates));
        }

        completions.push(self.updates.put(RawUpdates {
            txid: txid,
            updates: updates
        }));

        Completion { inner: Some(completions) }
    }

    /// Commits a typed transaction
    pub fn commit<S: Schema>(&mut self, tx: Transaction<S>) -> Completion {
        let mut completions = Vec::with_capacity(2);
        let mut updates = Vec::with_capacity(tx.next.len());
        let txid = tx.txid;

        completions.push(tx.commit(&mut updates));

        completions.push(self.updates.put(RawUpdates {
            txid: txid,
            updates: updates
        }));

        Completion { inner: Some(completions) }
    }
}

trait RawTable {
    fn commit_all_raw(
        &mut self,
        txid: u64,
        items: HashMap<String, Vec<Record>>,
        raw_updates: &mut Vec<RawUpdate>
    ) -> observe::Completion;
}

/// A raw transaction
pub struct RawTransaction {
    txid: u64,
    items: HashMap<String, HashMap<String, Vec<Record>>>,
}

impl RawTransaction {
    /// Creates a new raw transaction
    pub fn new() -> RawTransaction {
        RawTransaction {
            txid: random(),
            items: HashMap::new()
        }
    }

    /// Returns the ID of this transaction
    pub fn txid(&self) -> u64 {
        self.txid
    }

    /// Adds an item to this transaction. If an item with the given key already exists in this
    /// transaction, then the items will be merged when the transaction is committed.
    pub fn add(&mut self, table: String, k: String, data: Record) {
        self.items
            .entry(table).or_insert_with(|| HashMap::new())
            .entry(k).or_insert_with(|| Vec::new())
            .push(data);
    }
}

/// A table is a simple key-value mapping, where the keys are strings and the items are determined
/// by the schema.
pub struct Table<S: Schema> {
    inner: Rc<RefCell<TableInner<S>>>,
}

struct TableInner<S: Schema> {
    // I had kinda wanted to avoid this Rc<RefCell<Inner>> pattern if at all possible, but
    // couldn't make it work. This is only done so CRDT can hold a Box<RawTable> for performing
    // raw operations on the table. A proper Table cannot be cloned, so there will only ever
    // be up to two references to the TableInner floating around, which should hopefully make it
    // easier to reason about safely borrowing the contents

    name: String,
    schema: S,
    rows: HashMap<String, S::Item>,
    updates: Observable<Updates<S>>,
}

/// Schemas are the secret sauce that allow CRDB to function in an eventually consistent context.
/// Critically, schemas implement the `merge` operation that determines how two possibly divergent
/// states are merged together. See the module-level documentation for more information.
pub trait Schema {
    type Item: Clone + fmt::Debug;

    /// Encodes the item into a record
    fn encode(&self, item: &Self::Item) -> Record;

    /// Decodes an item from a record
    fn decode(&self, data: &Record) -> Self::Item;

    /// Updates item `a` by merging information from item `b`.
    ///
    /// This operation ***MUST*** be idempotent, associative, and commutative. See the module-level
    /// documentation for more information.
    fn merge(&self, a: Self::Item, b: Self::Item) -> Self::Item;
}

impl<S: Schema> Table<S> {
    /// Returns an `Observer` for the stream of updates to this table
    pub fn updates(&mut self) -> Observer<Updates<S>> {
        self.inner.borrow_mut().updates.observer()
    }

    /// Returns a copy of the data with the given key
    pub fn get<'t>(&'t self, k: &str) -> Option<S::Item> {
        self.inner.borrow().rows.get(k).cloned()
    }

    /// Creates a new typed transaction on this table.
    pub fn open<'t>(&'t mut self) -> Transaction<'t, S> {
        Transaction {
            txid: random(),
            inner: self.inner.borrow_mut(),
            next: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn snapshot(self) -> HashMap<String, S::Item> {
        self.inner.borrow().rows.clone()
    }
}

impl<S: Schema> RawTable for Table<S> {
    fn commit_all_raw(
        &mut self,
        txid: u64,
        items: HashMap<String, Vec<Record>>,
        raw_updates: &mut Vec<RawUpdate>
    ) -> observe::Completion {
        self.inner.borrow_mut().commit_all_raw(txid, items, raw_updates)
    }
}

impl<S: Schema> TableInner<S> {
    fn typed_update_as_raw(&self, update: &Update<S>) -> RawUpdate {
        RawUpdate {
            table: self.name.clone(),
            key: update.key.clone(),
            prev: update.prev.as_ref().map(|p| self.schema.encode(p)),
            item: self.schema.encode(&update.item),
        }
    }

    fn coalesce_raw(&self, rows: Vec<Record>) -> S::Item {
        assert!(rows.len() > 0);

        let item = rows.into_iter().fold(None, |cur, record| {
            let b = self.schema.decode(&record);

            if let Some(a) = cur {
                Some(self.schema.merge(a, b))
            } else {
                Some(b)
            }
        });

        item.unwrap()
    }

    fn commit_one(
        &mut self,
        key: String,
        item: S::Item,
        typed_updates: &mut Vec<Update<S>>,
        raw_updates: &mut Vec<RawUpdate>,
    ) {
        let prev = self.rows.remove(&key);
        let next = match prev {
            Some(ref prev) => self.schema.merge(prev.clone(), item),
            None => item,
        };

        self.rows.insert(key.clone(), next.clone());

        let typed_update = Update {
            key: key.clone(),
            prev: prev,
            item: next,
        };

        let raw_update = self.typed_update_as_raw(&typed_update);

        typed_updates.push(typed_update);
        raw_updates.push(raw_update);
    }

    fn commit_all_raw(
        &mut self,
        txid: u64,
        items: HashMap<String, Vec<Record>>,
        raw_updates: &mut Vec<RawUpdate>
    ) -> observe::Completion {
        let mut typed_updates = Vec::with_capacity(items.len());

        for (key, rows) in items.into_iter() {
            if rows.len() > 0 {
                let item = self.coalesce_raw(rows);
                self.commit_one(key, item, &mut typed_updates, raw_updates);
            }
        }

        self.updates.put(Updates {
            txid: txid,
            updates: typed_updates
        })
    }

    fn commit_all_typed(
        &mut self,
        txid: u64,
        items: HashMap<String, S::Item>,
        raw_updates: &mut Vec<RawUpdate>,
    ) -> observe::Completion {
        let mut typed_updates = Vec::with_capacity(items.len());

        for (key, item) in items.into_iter() {
            self.commit_one(key, item, &mut typed_updates, raw_updates);
        }

        self.updates.put(Updates {
            txid: txid,
            updates: typed_updates
        })
    }
}

/// A typed transaction on a single table
pub struct Transaction<'t, S: 'static + Schema> {
    txid: u64,
    inner: RefMut<'t, TableInner<S>>,
    next: HashMap<String, S::Item>,
}

impl<'t, S: 'static + Schema> Transaction<'t, S> {
    /// Returns the ID of this transaction
    pub fn txid(&self) -> u64 {
        self.txid
    }

    /// Reads an item from the table. This will behave as if any items added to the transaction
    /// have already been committed.
    pub fn get(&self, key: &str) -> Option<S::Item> {
        if let Some(prev) = self.inner.rows.get(key) {
            if let Some(next) = self.next.get(key) {
                Some(self.inner.schema.merge(prev.clone(), next.clone()))
            } else {
                Some(prev.clone())
            }
        } else {
            self.next.get(key).cloned()
        }
    }

    /// Adds an item to be merged when the transaction is complete
    pub fn add(&mut self, key: String, item: S::Item) {
        let next = match self.next.remove(&key) {
            Some(prev) => self.inner.schema.merge(prev, item),
            None => item,
        };

        self.next.insert(key, next);
    }

    /// Rolls back the transaction, discarding any updates that were added. The table is unchanged.
    pub fn rollback(self) {
        debug!("transaction {} rolled back", self.txid);
    }

    fn commit(mut self, raw_updates: &mut Vec<RawUpdate>) -> observe::Completion {
        self.inner.commit_all_typed(self.txid, self.next, raw_updates)
    }
}

/// A list of typed updates to a single table, generated as a result of committing a transaction.
pub struct Updates<S: Schema> {
    /// The ID of the transaction that generated this update
    pub txid: u64,
    /// The list of updated records applied as part of the transaction
    pub updates: Vec<Update<S>>,
}

/// A typed update to a single row
pub struct Update<S: Schema> {
    /// The key of the updated item
    pub key: String,
    /// The item that was replaced, if such an item exists
    pub prev: Option<S::Item>,
    /// The new item
    pub item: S::Item
}

impl<S: Schema> fmt::Debug for Updates<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Updates {{ txid: {}, updates: {:?} }}", self.txid, self.updates)
    }
}

impl<S: Schema> fmt::Debug for Update<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Update {{ key: {}, prev: {:?}, item: {:?} }}", self.key, self.prev, self.item)
    }
}

/// A list of raw updates to potentially multiple tables, generated as a result of committing a
/// transaction.
#[derive(Debug)]
pub struct RawUpdates {
    /// The ID of the transaction that generated this update
    pub txid: u64,
    /// The list of updated records applied as part of the transaction
    pub updates: Vec<RawUpdate>,
}

/// A raw update to a single row in a single table
#[derive(Debug)]
pub struct RawUpdate {
    /// The table of the updated item
    pub table: String,
    /// The key of the updated item
    pub key: String,
    /// The previous item, if such an item exists
    pub prev: Option<Record>,
    /// The new item
    pub item: Record,
}

/// A future that completes when a committed transaction has been observed by all observers.
pub struct Completion {
    inner: Option<Vec<observe::Completion>>
}

impl Future for Completion {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        let mut inner = match self.inner.take() {
            Some(inner) => inner,
            None => {
                warn!("Completion polled too many times");
                return Err(());
            }
        };

        while inner.len() > 0 {
            if let Async::Ready(_) = try!(inner[0].poll()) {
                inner.swap_remove(0);
            } else {
                self.inner = Some(inner);
                return Ok(Async::NotReady);
            }
        }

        self.inner = Some(inner);
        Ok(Async::Ready(()))
    }
}
