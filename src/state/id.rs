// state/id.rs -- unique ID strings
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Unique identifier strings

use std::cell;
use std::clone;
use std::cmp;
use std::fmt;
use std::hash;
use std::hash::Hash;
use std::marker::PhantomData;

use common::Sid;

// Using PhantomData like we do in this module allows us to construct distinct
// types for some arbitrary type we call the "namespace" of the Id while still
// allowing the same implementation for the different Ids.

/// A unique identifier, more or less a wrapper around a string with some extra
/// type information.
///
/// The type parameter can be any statically defined type to namespace the `Id`.
/// That is, an `Id<Foo>` cannot be assigned to an `Id<Bar>` if `Foo` and `Bar`
/// are distinct types. This is checked statically so there is no overhead at
/// run time.
///
/// To create an `Id`, use the `IdGenerator`
pub struct Id<Namespace: 'static> {
    id: String,
    _ns: PhantomData<&'static mut Namespace>
}

impl<Namespace> Id<Namespace> {
    /// Returns a byte array for this `Id`
    pub fn as_bytes(&self) -> &[u8] { self.id.as_bytes() }
}

impl<Namespace> fmt::Debug for Id<Namespace> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Id({})", self.id)
    }
}

impl<Namespace> clone::Clone for Id<Namespace> {
    fn clone(&self) -> Id<Namespace> {
        Id { id: self.id.clone(), _ns: PhantomData }
    }
}

impl<Namespace> cmp::PartialEq for Id<Namespace> {
    fn eq(&self, other: &Id<Namespace>) -> bool {
        self.id == other.id
    }
}

impl<Namespace> cmp::Eq for Id<Namespace> { }

impl<Namespace> Hash for Id<Namespace> {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.id.hash(state)
    }
}

/// An `Id` generator. This is the only way to create an `Id`
///
/// The type parameter determines what kinds of `Id` are generated. That is,
/// an `IdGenerator<Foo>` cannot generate an `Id<Bar>`.
pub struct IdGenerator<Namespace: 'static> {
    sid: Sid,
    next: cell::Cell<u64>,
    _ns: PhantomData<&'static mut Namespace>
}

impl<Namespace> IdGenerator<Namespace> {
    /// Creates a new `Id` generator
    pub fn new(sid: Sid) -> IdGenerator<Namespace> {
        IdGenerator {
            sid: sid,
            next: cell::Cell::new(0),
            _ns: PhantomData,
        }
    }

    /// Generates the next `Id`
    pub fn next(&self) -> Id<Namespace> {
        let id = self.next.get();
        self.next.set(id + 1);

        Id {
            id: format!("{}:{}", self.sid, id),
            _ns: PhantomData
        }
    }
}

#[cfg(test)]
struct Foo;

#[cfg(test)]
struct Bar;

#[test]
fn test_types_ok() {
    let fooid: IdGenerator<Foo> = IdGenerator::new(Sid::identity());
    let barid: IdGenerator<Bar> = IdGenerator::new(Sid::identity());

    let _: Id<Foo> = fooid.next();
    let _: Id<Bar> = barid.next();
}
