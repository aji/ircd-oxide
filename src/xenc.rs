// xenc.rs -- the XENC format
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::convert::From;
use std::io;
use std::io::prelude::*;
use std::result;
use time::Timespec;

/// A simple error type
#[derive(Debug, PartialEq, Eq)]
pub struct Error;

/// A result type
pub type Result<T> = result::Result<T, Error>;

/// An XENC value.
///
/// `Value`s are the nodes in the XENC parse tree. `I64` and `Octets` are always
/// leaves, while `List` and `Dict` may contain other values.
#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    I64(i64),
    Time(Timespec),
    Octets(Vec<u8>),
    List(Vec<Value>),
    Dict(HashMap<Vec<u8>, Value>),
}

impl Value {
    /// The contained value as an `i16`, if `self` is an `I64`, otherwise
    /// `None`
    pub fn into_i64(self) -> Option<i64> {
        match self { Value::I64(v) => Some(v), _ => None }
    }

    /// The contained value as a `Timespec`, if `self` is a `Time`, otherwise
    /// `None`
    pub fn into_time(self) -> Option<Timespec> {
        match self { Value::Time(t) => Some(t), _ => None }
    }

    /// The contained value as a vector of octets, if `self` is an `Octets`,
    /// otherwise `None`.
    pub fn into_octets(self) -> Option<Vec<u8>> {
        match self { Value::Octets(v) => Some(v), _ => None }
    }

    /// The contained value as a vector of `Value`, if `self` is a `List`,
    /// otherwise `None`.
    pub fn into_list(self) -> Option<Vec<Value>> {
        match self { Value::List(v) => Some(v), _ => None }
    }

    /// The contained value as a map of `Vec<u8>` to `Value, if `self` is a
    /// `Dict`, otherwise `None`.
    pub fn into_dict(self) -> Option<HashMap<Vec<u8>, Value>> {
        match self { Value::Dict(v) => Some(v), _ => None }
    }

    /// Serializes `self` to the given `Write`able, otherwise `None`.
    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match *self {
            Value::I64(v) => write!(w, "i{}e", v),

            Value::Time(t) => write!(w, "t{}.{}e", t.sec, t.nsec),

            Value::Octets(ref v) => {
                try!(write!(w, "{}:", v.len()));
                w.write_all(&v[..])
            },

            Value::List(ref v) => {
                try!(write!(w, "l"));
                for child in v.iter() {
                    try!(child.write(w));
                }
                try!(write!(w, "e"));
                Ok(())
            },

            Value::Dict(ref v) => {
                try!(write!(w, "d"));
                for (k, child) in v.iter() {
                    try!(write!(w, "{}:", k.len()));
                    try!(w.write_all(&k[..]));
                    try!(child.write(w));
                }
                try!(write!(w, "e"));
                Ok(())
            },
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Value {
        Value::I64(v)
    }
}

impl From<Timespec> for Value {
    fn from(v: Timespec) -> Value {
        Value::Time(v)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Value {
        Value::Octets(v)
    }
}

impl<T> From<Vec<T>> for Value where Value: From<T> {
    fn from(v: Vec<T>) -> Value {
        Value::List(v.into_iter().map(|w| From::from(w)).collect())
    }
}

impl<T> From<HashMap<Vec<u8>, T>> for Value where Value: From<T> {
    fn from(v: HashMap<Vec<u8>, T>) -> Value {
        Value::Dict(v.into_iter().map(|(k, v)| (k, From::from(v))).collect())
    }
}

/// A trait for things that can be deserialized from XENC values
pub trait FromXenc: Sized {
    fn from_xenc(v: Value) -> Result<Self>;
}

impl FromXenc for i64 {
    fn from_xenc(v: Value) -> Result<i64> {
        v.into_i64().ok_or(Error)
    }
}

impl FromXenc for Timespec {
    fn from_xenc(v: Value) -> Result<Timespec> {
        v.into_time().ok_or(Error)
    }
}

impl FromXenc for Vec<u8> {
    fn from_xenc(v: Value) -> Result<Vec<u8>> {
        v.into_octets().ok_or(Error)
    }
}

impl FromXenc for Vec<Value> {
    fn from_xenc(v: Value) -> Result<Vec<Value>> {
        v.into_list().ok_or(Error)
    }
}

impl FromXenc for HashMap<Vec<u8>, Value> {
    fn from_xenc(v: Value) -> Result<HashMap<Vec<u8>, Value>> {
        v.into_dict().ok_or(Error)
    }
}

/// A parser
pub struct Parser<'a> {
    buf: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    /// Creates a new parser over the given byte slice
    pub fn new(buf: &[u8]) -> Parser {
        Parser { buf: buf, i: 0 }
    }

    /// Checks if the parser is at the end of its input
    pub fn empty(&self) -> bool {
        self.i >= self.buf.len()
    }

    fn peek(&self) -> u8 {
        self.buf[self.i] // XXX: could panic!
    }

    fn getch(&mut self) -> u8 {
        self.i += 1;
        self.buf[self.i - 1] // XXX: could panic!
    }

    fn read_i64(&mut self, delim: u8) -> Result<i64> {
        let mut v = 0;

        let neg = match self.peek() {
            b'-' => { self.getch(); true },
            _ => false,
        };

        while !self.empty() {
            match self.getch() {
                d@b'0'...b'9' => {
                    v = (v * 10) + (d - b'0') as i64
                },
                x if x == delim => break,
                _ => return Err(Error) // invalid int character
            }
        }

        if neg {
            v = -v;
        }

        Ok(v)
    }

    /// Fetches the next `Value` in the input slice, or an error if there was a
    /// problem with the data.
    pub fn next(&mut self) -> Result<Value> {
        match self.peek() {
            b'i' => {
                self.getch();
                Ok(Value::I64(try!(self.read_i64(b'e'))))
            },

            b't' => {
                self.getch();
                let sec = try!(self.read_i64(b'.'));
                let nsec = try!(self.read_i64(b'e')) as i32;
                Ok(Value::Time(Timespec::new(sec, nsec)))
            },

            b'0'...b'9' => {
                let len = try!(self.read_i64(b':')) as usize;

                let start = self.i;
                let end = self.i + len;

                if end < start || end > self.buf.len() {
                    Err(Error) // invalid length
                } else {
                    self.i = end;
                    Ok(Value::Octets(self.buf[start..end].to_owned()))
                }
            },

            b'l' => {
                let mut v = Vec::new();
                self.getch();
                while !self.empty() {
                    if b'e' == self.peek() {
                        self.getch();
                        return Ok(Value::List(v));
                    } else {
                        v.push(try!(self.next()))
                    }
                }
                Err(Error) // missing 'e'
            },

            b'd' => {
                let mut v = HashMap::new();
                self.getch();
                while !self.empty() {
                    if b'e' == self.peek() {
                        self.getch();
                        return Ok(Value::Dict(v));
                    } else {
                        let k = match try!(self.next()) {
                            Value::Octets(k) => k,
                            _ => return Err(Error) // non-string key
                        };
                        v.insert(k, try!(self.next()));
                    }
                }
                Err(Error) // missing 'e'
            },

            _ => Err(Error)
        }
    }
}

#[cfg(test)]
fn decode(s: &str) -> Result<Value> {
    Parser::new(s.as_bytes()).next()
}

#[cfg(test)]
fn codec(s1: &str) -> bool {
    let v1 = Parser::new(s1.as_bytes()).next().unwrap();

    let s2 = {
        let mut s = Vec::new();
        v1.write(&mut s).unwrap();
        s
    };

    let v2 = Parser::new(&s2[..]).next().unwrap();

    println!("v1 = {:?}", v1);
    println!("v2 = {:?}", v2);

    v1 == v2
}

#[test]
fn test_integers() {
    assert_eq!(Ok(Value::I64(0)),    decode("i0e"));
    assert_eq!(Ok(Value::I64(6)),    decode("i6e"));
    assert_eq!(Ok(Value::I64(10)),   decode("i10e"));
    assert_eq!(Ok(Value::I64(37)),   decode("i37e"));
    assert_eq!(Ok(Value::I64(-6)),   decode("i-6e"));
    assert_eq!(Ok(Value::I64(-37)),  decode("i-37e"));
    assert_eq!(Err(Error),           decode("i?e"));
}

#[test]
fn test_times() {
    assert_eq!(
        Ok(Value::Time(Timespec { sec: 5, nsec: 10 })),
        decode("t5.10e")
    );
}

#[test]
fn test_strings() {
    assert_eq!(Ok(Value::Octets(b"123".to_vec())),  decode("3:123"));
    assert_eq!(Ok(Value::Octets(b"123".to_vec())),  decode("3:123junk"));
    assert_eq!(Err(Error),                          decode("3:12"));
}

#[test]
fn test_simple_list() {
    assert_eq!(
        Ok(Value::List(vec![
            Value::I64(3),
            Value::Octets(b"123".to_vec()),
            Value::I64(-10),
        ])),
        decode("li3e3:123i-10ee")
    );

    assert_eq!(Err(Error), decode("li3e"));

    let mut p = Parser::new(b"lei0e");
    assert_eq!(Value::List(Vec::new()),  p.next().unwrap());
    assert_eq!(Value::I64(0),            p.next().unwrap());
}

#[test]
fn test_nested_list() {
    assert_eq!(
        Ok(Value::List(vec![
            Value::I64(3),
            Value::List(vec![
                Value::List(vec![
                    Value::I64(4),
                ]),
                Value::I64(5),
                Value::I64(6),
            ]),
            Value::I64(7),
        ])),
        decode("li3elli4eei5ei6eei7ee")
    );
}

#[test]
fn test_very_nested_list() {
    assert_eq!(
        Ok(Value::List(vec![
            Value::List(vec![
                Value::List(vec![
                    Value::List(vec![
                        Value::List(vec![
                            Value::List(vec![
                            ]),
                        ]),
                    ]),
                ]),
            ]),
        ])),
        decode("lllllleeeeee")
    );
}

#[test]
fn test_simple_dict() {
    let mut d = HashMap::new();
    d.insert(b"abc".to_vec(), Value::I64(3));
    d.insert(b"def".to_vec(), Value::Octets(b"123".to_vec()));

    assert_eq!(
        Ok(Value::Dict(d)),
        decode("d3:abci3e3:def3:123e")
    );

    assert_eq!(Err(Error), decode("d3:abce")); // missing value
    assert_eq!(Err(Error), decode("d3:abci0e")); // end of input
    assert_eq!(Err(Error), decode("di0ei0ee")); // non-string key

    let mut p = Parser::new(b"dei0e");
    assert_eq!(Value::Dict(HashMap::new()),  p.next().unwrap());
    assert_eq!(Value::I64(0),                p.next().unwrap());

    // We don't really need to test if nesting works properly on dicts!
    // at this point we know that only strings are allowed as keys, that
    // we are using .next() to get keys and values, and that the dict
    // variant of .next() leaves the pointer in the right spot.
}

#[test]
fn test_codecs() {
    assert!(codec("i6e"));                    // 6
    assert!(codec("t5.10e"));                 // json has no native time
    assert!(codec("3:abc"));                  // "abc"
    assert!(codec("le"));                     // []
    assert!(codec("li6e3:abce"));             // [6,"abc"]
    assert!(codec("li6el3:abcee"));           // [6,["abc"]]
    assert!(codec("de"));                     // {}
    assert!(codec("d3:abc3:defe"));           // {"abc":"def"}
    assert!(codec("d3:abcd3:defi6eee"));      // {"abc":{"def":6}}
}
