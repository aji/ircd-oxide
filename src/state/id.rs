// state/id.rs -- unique ID strings
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>

//! Unique identifier strings

use std::clone;
use std::cmp;
use std::fmt;
use std::marker::PhantomData;

use state::clock::Sid;

// Using PhantomData like we do in this module allows us to make Id covariant
// with some arbitrary type, which we call the "namespace" of the Id.

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

/// An `Id` generator. This is the only way to create an `Id`
///
/// The type parameter determines what kinds of `Id` are generated. That is,
/// an `IdGenerator<Foo>` cannot generate an `Id<Bar>`.
pub struct IdGenerator<Namespace: 'static> {
    sid: Sid,
    next: u64,
    _ns: PhantomData<&'static mut Namespace>
}

impl<Namespace> IdGenerator<Namespace> {
    /// Creates a new `Id` generator
    pub fn new(sid: Sid) -> IdGenerator<Namespace> {
        IdGenerator {
            sid: sid,
            next: 0,
            _ns: PhantomData,
        }
    }

    /// Generates the next `Id`
    pub fn next(&mut self) -> Id<Namespace> {
        let id = self.next;
        self.next += 1;

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
    let mut fooid: IdGenerator<Foo> = IdGenerator::new(0);
    let mut barid: IdGenerator<Bar> = IdGenerator::new(0);

    let _: Id<Foo> = fooid.next();
    let _: Id<Bar> = barid.next();
}
