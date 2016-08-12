// irc/message.rs -- message parsing
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide, and is protected under the terms contained
// in the COPYING file in the project root.

//! Message parsing

use std::fmt;

pub type ParseResult<T> = Result<T, &'static str>;

/// Helper for the message parser
struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

/// The parsed form of an IRC message.
#[derive(PartialEq)]
pub struct Message<'a> {
    /// The verb portion of a message, specifying which action to take.
    pub verb:  &'a str,
    /// The arguments to the verb.
    pub args:  Vec<&'a str>,
}

impl<'a> Scanner<'a> {
    fn new(s: &[u8]) -> Scanner {
        Scanner {
            s: s,
            i: 0,
        }
    }

    fn peek(&self) -> u8 {
        if self.i < self.s.len() {
            self.s[self.i]
        } else {
            0
        }
    }

    fn empty(&self) -> bool {
        self.i >= self.s.len()
    }

    fn skip(&mut self) {
        self.i += 1;
    }

    fn skip_spaces(&mut self) {
        while !self.empty() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn chomp(&mut self) -> ParseResult<&'a str> {
        self.skip_spaces();
        let start = self.i;
        while !self.empty() && !(self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
        let end = self.i;
        self.skip_spaces();

        match ::std::str::from_utf8(&self.s[start..end]) {
            Ok(s) => Ok(s),
            Err(_) => Err("slice is not valid UTF-8"),
        }
    }

    fn chomp_remaining(&mut self) -> ParseResult<&'a str> {
        let i = self.i;
        self.i = self.s.len();

        match ::std::str::from_utf8(&self.s[i..]) {
            Ok(s) => Ok(s),
            Err(_) => Err("slice is not valid UTF-8"),
        }
    }
}

impl<'a> Message<'a> {
    /// Parses the byte slice into a `Message`
    pub fn parse(spec: &'a [u8]) -> ParseResult<Message<'a>> {
        let mut scan = Scanner::new(spec);

        scan.skip_spaces();

        let verb = try!(scan.chomp());

        let mut args = Vec::new();
        while !scan.empty() {
            args.push(if scan.peek() == b':' {
                scan.skip();
                try!(scan.chomp_remaining())
            } else {
                try!(scan.chomp())
            });
        }

        Ok(Message {
            verb: verb,
            args: args
        })
    }
}

impl<'a> fmt::Debug for Message<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Message({:?}", self.verb));
        for s in self.args.iter() {
            try!(write!(f, ", {:?}", s));
        }
        try!(write!(f, ")"));
        Ok(())
    }
}

#[test]
fn message_parse_easy() {
    assert_eq!(Message {
        verb: "PING",
        args: vec!["123"],
    }, Message::parse(b"PING 123").unwrap());
}

#[test]
fn message_parse_trailing() {
    assert_eq!(Message {
        verb: "PING",
        args: vec!["this has spaces"],
    }, Message::parse(b"PING :this has spaces").unwrap());
}

#[test]
fn message_parse_with_spaces() {
    assert_eq!(Message {
        verb: "PING",
        args: vec!["this", "has", "spaces"],
    }, Message::parse(b"PING this has spaces").unwrap());
}

#[test]
fn message_parse_dumb_client() {
    assert_eq!(Message {
        verb: "PING",
        args: vec!["this", "has", "spaces  "],
    }, Message::parse(b"   PING       this  has :spaces  ").unwrap());
}

#[test]
fn message_parse_client_still_dumb() {
    assert_eq!(Message {
        verb: "PING",
        args: vec!["this", "has", "spaces"],
    }, Message::parse(b"   PING this has spaces          ").unwrap());
}

#[test]
fn message_parse_invalid_utf8() {
    assert!(Message::parse(b"\xff\xff\xff\xff").is_err());
}
