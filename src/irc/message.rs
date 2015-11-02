// irc/message.rs -- message parsing
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide, and is protected under the terms contained
// in the COPYING file in the project root.

//! Message parsing

use std::fmt;

/// Helper for the message parser
struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

/// The parsed form of an IRC message.
#[derive(PartialEq)]
pub struct Message<'a> {
    /// The verb portion of a message, specifying which action to take.
    pub verb:  &'a [u8],
    /// The arguments to the verb.
    pub args:  Vec<&'a [u8]>,
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

    fn chomp(&mut self) -> &'a [u8] {
        self.skip_spaces();
        let start = self.i;
        while !self.empty() && !(self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
        let end = self.i;
        self.skip_spaces();

        &self.s[start..end]
    }

    fn chomp_remaining(&mut self) -> &'a [u8] {
        let i = self.i;
        self.i = self.s.len();
        &self.s[i..]
    }
}

impl<'a> Message<'a> {
    /// Parses the byte slice into a `Message`
    pub fn parse(spec: &'a [u8]) -> Result<Message<'a>, &'static str> {
        let mut scan = Scanner::new(spec);

        scan.skip_spaces();

        let verb = scan.chomp();

        let mut args = Vec::new();
        while !scan.empty() {
            args.push(if scan.peek() == b':' {
                scan.skip();
                scan.chomp_remaining()
            } else {
                scan.chomp()
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
        try!(write!(f, "Message({:?}", String::from_utf8_lossy(self.verb)));
        for s in self.args.iter() {
            try!(write!(f, ", {:?}", String::from_utf8_lossy(s)));
        }
        try!(write!(f, ")"));
        Ok(())
    }
}

#[test]
fn message_parse_easy() {
    assert_eq!(Message {
        verb: b"PING",
        args: vec![b"123"],
    }, Message::parse(b"PING 123").unwrap());
}

#[test]
fn message_parse_trailing() {
    assert_eq!(Message {
        verb: b"PING",
        args: vec![b"this has spaces"],
    }, Message::parse(b"PING :this has spaces").unwrap());
}

#[test]
fn message_parse_with_spaces() {
    assert_eq!(Message {
        verb: b"PING",
        args: vec![b"this", b"has", b"spaces"],
    }, Message::parse(b"PING this has spaces").unwrap());
}

#[test]
fn message_parse_dumb_client() {
    assert_eq!(Message {
        verb: b"PING",
        args: vec![b"this", b"has", b"spaces"],
    }, Message::parse(b"   PING       this  has :spaces").unwrap());
}

#[test]
fn message_parse_client_still_dumb() {
    assert_eq!(Message {
        verb: b"PING",
        args: vec![b"this", b"has", b"spaces"],
    }, Message::parse(b"   PING this has spaces          ").unwrap());
}
