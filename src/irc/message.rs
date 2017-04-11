// irc/message.rs -- message parsing
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide, and is protected under the terms contained
// in the COPYING file in the project root.

//! Message parsing

use std::fmt;

use bytes::Bytes;

pub type ParseResult<T> = Result<T, &'static str>;

/// Helper for the message parser
struct Scanner {
    b: Bytes,
}

/// The parsed form of an IRC message.
#[derive(PartialEq)]
pub struct Message {
    /// The verb portion of a message, specifying which action to take.
    pub verb: Bytes,
    /// The arguments to the verb.
    pub args: Vec<Bytes>,
}

impl Scanner {
    fn new(b: Bytes) -> Scanner {
        Scanner { b: b }
    }

    fn peek(&self) -> u8 {
        self.b.first().cloned().unwrap_or(0)
    }

    fn empty(&self) -> bool {
        self.b.is_empty()
    }

    fn skip(&mut self) {
        self.b.split_to(1);
    }

    fn skip_spaces(&mut self) {
        let end = self.b.iter()
            .position(|c| !(*c as char).is_whitespace())
            .unwrap_or(self.b.len());

        self.b.split_to(end);
    }

    fn chomp(&mut self) -> Bytes {
        self.skip_spaces();

        let end = self.b.iter()
            .position(|c| (*c as char).is_whitespace())
            .unwrap_or(self.b.len());

        let buf = self.b.split_to(end);
        self.skip_spaces();
        buf
    }

    fn chomp_remaining(&mut self) -> Bytes {
        let end = self.b.len();
        self.b.split_to(end)
    }
}

impl Message {
    /// Parses the byte slice into a `Message`
    pub fn parse<T>(spec: T) -> ParseResult<Message>
    where Bytes: From<T> {
        let mut scan = Scanner::new(From::from(spec));

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

fn write_bytes(f: &mut fmt::Formatter, s: &Bytes) -> fmt::Result {
    match ::std::str::from_utf8(&s[..]) {
        Ok(t) => write!(f, "{:?}", t),
        Err(_) => write!(f, "{:?}", s)
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Message("));
        try!(write_bytes(f, &self.verb));

        for s in self.args.iter() {
            try!(write!(f, ", "));
            try!(write_bytes(f, s));
        }

        try!(write!(f, ")"));
        Ok(())
    }
}

#[cfg(test)]
fn test_good_parse(
    line: &str,
    verb: &str,
    args: Vec<&str>
) {
    let expected = Message {
        verb: Bytes::from(verb),
        args: args.into_iter().map(|v| Bytes::from(v)).collect()
    };

    let actual = Message::parse(&line[..]).unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn message_parse_easy() {
    test_good_parse(
        "PING 123",
        "PING", vec!["123"]
    );
}

#[test]
fn message_parse_trailing() {
    test_good_parse(
        "PING :this has spaces",
        "PING", vec!["this has spaces"],
    );
}

#[test]
fn message_parse_trailing_extra_space() {
    test_good_parse(
        "PING this :   has spaces",
        "PING", vec!["this", "   has spaces"],
    );
}

#[test]
fn message_parse_with_spaces() {
    test_good_parse(
        "PING this has spaces",
        "PING", vec!["this", "has", "spaces"],
    );
}

#[test]
fn message_parse_dumb_client() {
    test_good_parse(
        "   PING       this  has :spaces  ",
        "PING", vec!["this", "has", "spaces  "],
    );
}

#[test]
fn message_parse_client_still_dumb() {
    test_good_parse(
        "   PING this has spaces          ",
        "PING", vec!["this", "has", "spaces"],
    );
}
