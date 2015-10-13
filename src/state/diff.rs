// state/diff.rs -- state differences
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! Definitions for difference computations

use irc::IrcString;

/// A trait that indicates a difference may be computed between elements of
/// this type.
pub trait Diffable<'r> {
    /// The atomic element of a difference. Items may be added, removed, or
    /// changed between revisions of the Diffable.
    type Item: 'r;

    /// The type of difference that is produced by comparison.
    type Diff: 'r + IntoIterator<Item=Differ<&'r Self::Item>>;

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

/// A default implementation of `Diffable` that can be used on atomic items
pub trait AtomicDiffable: Eq { }

impl<'r, A: 'r> Diffable<'r> for A where A: AtomicDiffable {
    type Item = Self;
    type Diff = Option<Differ<&'r Self>>;

    fn diff(&'r self, new: &'r Self) -> Self::Diff {
        if self == new {
            None
        } else {
            Some(Differ::Changed(self, new))
        }
    }
}

impl AtomicDiffable for i8  { }
impl AtomicDiffable for i16 { }
impl AtomicDiffable for i32 { }
impl AtomicDiffable for i64 { }

impl AtomicDiffable for u8  { }
impl AtomicDiffable for u16 { }
impl AtomicDiffable for u32 { }
impl AtomicDiffable for u64 { }

impl AtomicDiffable for String { }
impl AtomicDiffable for IrcString { }

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
