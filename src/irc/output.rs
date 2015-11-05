// irc/output.rs -- IRC output handling
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! IRC output handling

use std::io;
use std::fmt::Arguments;
use std::convert::AsRef;

/// A trait for things that can be written to with IRC lines
pub trait IrcWriter {
    /// Writes an IRC line with the verb and format arguments. Use the `irc!`
    /// macro for calling this.
    fn irc<'a, V>(&mut self, verb: &V, args: Arguments<'a>) -> io::Result<()>
    where V: AsRef<[u8]>;
}

impl<W> IrcWriter for W where W: io::Write {
    fn irc<'a, V>(&mut self, verb: &V, args: Arguments<'a>) -> io::Result<()>
    where V: AsRef<[u8]> {
        try!(self.write(verb.as_ref()));
        try!(self.write_fmt(format_args!(" {}\r\n", args)));
        Ok(())
    }
}

#[test]
fn write_format() {
    let mut s: Vec<u8> = Vec::new();

    irc!(&mut s, "PING", ":{}", 3).unwrap();

    assert_eq!("PING :3\r\n".as_bytes(), &s[..]);
}
