// state/diff.rs -- state differences
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! Difference computations

use std::collections::HashMap;
use std::hash::Hash;

use irc::IrcString;

/// A trait that indicates a difference may be computed between elements of
/// this type.
pub trait Diffable<'r> {
    /// The atomic element of a difference. Items may be added, removed, or
    /// changed between revisions of the Diffable.
    type Item: 'r;

    /// The type of difference that is produced by comparison.
    type Diff: 'r + IntoIterator<Item=Differ<Self::Item>>;

    /// Compares `self` to `new`, returning an instance of `Diff` that
    /// indicates which `Item`s have been added, removed, or changed in the
    /// new revision.
    fn diff(&'r self, new: &'r Self) -> Self::Diff;
}

/// Used to signal how an item has changed between revisions.
#[derive(Debug, PartialEq, Eq)]
pub enum Differ<I> {
    Added(I),
    Removed(I),
    Changed(I, I),
}

impl<'r, K: 'r, V: 'r> Diffable<'r> for HashMap<K, V>
where K: Eq + Hash, V: Eq {
    type Item = (&'r K, &'r V);
    type Diff = Vec<Differ<Self::Item>>;

    fn diff(&'r self, new: &'r Self) -> Self::Diff {
        use self::Differ::*;

        let mut diff = Vec::new();

        // check if anything has been removed or changed by iterating over our
        // own set first
        for (k, v1) in self.iter() {
            match new.get(k) {
                Some(v2) if v1 == v2 => { },
                Some(v2) if v1 != v2 => diff.push(Changed((k, v1), (k, v2))),
                Some(_) => panic!(),
                None => diff.push(Removed((k, v1))),
            }
        }

        // then check if anything has been added by iterating over the new set
        for (k, v2) in new.iter() {
            match self.get(k) {
                Some(_) => { },
                None => diff.push(Added((k, v2)))
            }
        }

        diff
    }
}

/// A default implementation of `Diffable` that can be used on atomic items,
/// i.e. items that don't contain any further items to consider differences of.
/// Only ever returns a single `Differ::Changed` for the item. This is
/// essentially a `Diffable`-flavored wrapper around `Eq`
///
/// # Example
///
/// ```rust
/// let a = "Hello".to_owned();
/// let b = "world".to_owned();
///
/// a.diff(&b); // returns Some(Changed(..)) with references to a and b
/// ```
pub trait AtomDiffable: Eq { }

impl<'r, A: 'r> Diffable<'r> for A where A: AtomDiffable {
    type Item = &'r Self;
    type Diff = Option<Differ<&'r Self>>;

    fn diff(&'r self, new: &'r Self) -> Self::Diff {
        if self == new {
            None
        } else {
            Some(Differ::Changed(self, new))
        }
    }
}

impl AtomDiffable for i8  { }
impl AtomDiffable for i16 { }
impl AtomDiffable for i32 { }
impl AtomDiffable for i64 { }

impl AtomDiffable for u8  { }
impl AtomDiffable for u16 { }
impl AtomDiffable for u32 { }
impl AtomDiffable for u64 { }

impl AtomDiffable for String { }
impl AtomDiffable for IrcString { }

#[test]
fn test_hashmap_diffable_added() {
    use self::Differ::*;

    let old = {
        let mut old = HashMap::new();
        old.insert(1, 2);
        old.insert(3, 4);
        old
    };

    let new = {
        let mut new = HashMap::new();
        new.insert(1, 2);
        new.insert(3, 4);
        new.insert(5, 6);
        new
    };

    assert_eq!(vec![Added((&5, &6))], old.diff(&new));
}

#[test]
fn test_hashmap_diffable_removed() {
    use self::Differ::*;

    let old = {
        let mut old = HashMap::new();
        old.insert(1, 2);
        old.insert(3, 4);
        old.insert(5, 6);
        old
    };

    let new = {
        let mut new = HashMap::new();
        new.insert(1, 2);
        new.insert(3, 4);
        new
    };

    assert_eq!(vec![Removed((&5, &6))], old.diff(&new));
}

#[test]
fn test_hashmap_diffable_changed() {
    use self::Differ::*;

    let old = {
        let mut old = HashMap::new();
        old.insert(1, 2);
        old.insert(3, 4);
        old
    };

    let new = {
        let mut new = HashMap::new();
        new.insert(1, 2);
        new.insert(3, 7);
        new
    };

    assert_eq!(vec![Changed((&3, &4), (&3, &7))], old.diff(&new));
}

#[test]
fn test_atomic_diffable() {
    use self::Differ::*;

    assert_eq!(Some(Changed(&2u32, &4u32)), 2u32.diff(&4u32));
    assert_eq!(Some(Changed(&4u32, &2u32)), 4u32.diff(&2u32));
    assert_eq!(None, (&2u32).diff(&2u32));

    let xstr = "x".to_owned();
    let ystr = "y".to_owned();
    assert_eq!(Some(Changed(&xstr, &ystr)), xstr.diff(&ystr));
    assert_eq!(Some(Changed(&ystr, &xstr)), ystr.diff(&xstr));
    assert_eq!(None, xstr.diff(&xstr));
}
