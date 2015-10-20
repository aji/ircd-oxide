// xenc.rs -- the XENC format
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    I64(i64),
    Octets(Vec<u8>),
    List(Vec<Value>),
    Dict(HashMap<Vec<u8>, Value>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct XencError;

pub struct Parser<'a> {
    buf: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    pub fn new(buf: &[u8]) -> Parser {
        Parser { buf: buf, i: 0 }
    }

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

    fn read_i64(&mut self, delim: u8) -> Result<i64, XencError> {
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
                _ => return Err(XencError) // invalid int character
            }
        }

        if neg {
            v = -v;
        }

        Ok(v)
    }

    pub fn next(&mut self) -> Result<Value, XencError> {
        match self.peek() {
            b'i' => {
                self.getch();
                Ok(Value::I64(try!(self.read_i64(b'e'))))
            },

            b'0'...b'9' => {
                let len = try!(self.read_i64(b':')) as usize;

                let start = self.i;
                let end = self.i + len;

                if end < start || end > self.buf.len() {
                    Err(XencError) // invalid length
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
                Err(XencError) // missing 'e'
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
                            _ => return Err(XencError) // non-string key
                        };
                        v.insert(k, try!(self.next()));
                    }
                }
                Err(XencError) // missing 'e'
            },

            _ => Err(XencError)
        }
    }
}

#[cfg(test)]
fn decode(s: &str) -> Result<Value, XencError> {
    Parser::new(s.as_bytes()).next()
}

#[test]
fn test_integers() {
    assert_eq!(Ok(Value::I64(0)),    decode("i0e"));
    assert_eq!(Ok(Value::I64(6)),    decode("i6e"));
    assert_eq!(Ok(Value::I64(10)),   decode("i10e"));
    assert_eq!(Ok(Value::I64(37)),   decode("i37e"));
    assert_eq!(Ok(Value::I64(-6)),   decode("i-6e"));
    assert_eq!(Ok(Value::I64(-37)),  decode("i-37e"));
    assert_eq!(Err(XencError),       decode("i?e"));
}

#[test]
fn test_strings() {
    assert_eq!(Ok(Value::Octets(b"123".to_vec())),  decode("3:123"));
    assert_eq!(Ok(Value::Octets(b"123".to_vec())),  decode("3:123junk"));
    assert_eq!(Err(XencError),                      decode("3:12"));
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

    assert_eq!(Err(XencError), decode("li3e"));
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

    assert_eq!(Err(XencError), decode("d3:abce")); // missing value
    assert_eq!(Err(XencError), decode("d3:abci0e")); // end of input
    assert_eq!(Err(XencError), decode("di0ei0ee")); // non-string key
}
