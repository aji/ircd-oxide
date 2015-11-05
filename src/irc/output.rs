// irc/output.rs -- IRC output handling
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! IRC output handling

use std::fmt::Arguments;
use std::convert::AsRef;

/// A trait for things that can be written to with IRC lines
pub trait IrcWriter {
    /// Writes an IRC line with the verb and format arguments. Use the `irc!`
    /// macro for calling this.
    fn irc<'a, V>(&mut self, verb: &V, args: Arguments<'a>)
    where V: AsRef<[u8]>;
}

/// Writes the IRC line to the given `IrcWriter`. Invokes `format_args!`
macro_rules! irc {
    ($irc:expr, $verb:expr, $($args:tt)*) => {
        $crate::irc::output::IrcWriter::irc(
            $irc, &$verb, format_args!($($args)*)
        )
    }
}

impl IrcWriter for Vec<u8> {
    fn irc<'a, V: AsRef<[u8]>>(&mut self, verb: &V, args: Arguments<'a>) {
        self.extend(verb.as_ref());

        let s = format!(" {}", args);

        if s.len() > 1 { // 1 for the space
            self.extend(s.as_bytes());
        }

        self.extend("\r\n".as_bytes());
    }
}

#[test]
fn write_no_space() {
    let mut s: Vec<u8> = Vec::new();

    irc!(&mut s, "PING", "");

    assert_eq!("PING\r\n".as_bytes(), &s[..]);
}

#[test]
fn write_space() {
    let mut s: Vec<u8> = Vec::new();

    irc!(&mut s, "PING", ":pong");

    assert_eq!("PING :pong\r\n".as_bytes(), &s[..]);
}

#[test]
fn write_format() {
    let mut s: Vec<u8> = Vec::new();

    irc!(&mut s, "PING", ":{}", 3);

    assert_eq!("PING :3\r\n".as_bytes(), &s[..]);
}
