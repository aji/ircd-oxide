// irc/output.rs -- a module for formatting IRC messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Formatting IRC messages

use irc::numeric::Numeric;
use irc::net::IrcStream;

/// A formatter for IRC lines
pub struct IrcFormatter {
    server: Vec<u8>
}

impl IrcFormatter {
    /// Creates a new formatter using the given server name for
    /// server-originated messages.
    pub fn new(server: &[u8]) -> IrcFormatter {
        IrcFormatter { server: server.to_vec() }
    }

    /// Creates a writer to the given IRC stream that will use this IRC
    /// formatter.
    pub fn writer<'w, 'fmt, 'sock>(&'fmt self, sock: &'sock IrcStream)
    -> IrcWriter<'w> where 'fmt: 'w, 'sock: 'w {
        IrcWriter::new(self, sock)
    }
}

/// A writer to an IRC stream, derived from an IRC formatter
pub struct IrcWriter<'w> {
    fmt: &'w IrcFormatter,
    sock: &'w IrcStream,
}

impl<'w> IrcWriter<'w> {
    fn new(fmt: &'w IrcFormatter, sock: &'w IrcStream) -> IrcWriter<'w> {
        IrcWriter { fmt: fmt, sock: sock }
    }
}

fn splice<'o, 'i, O, I>(to: &mut O, mut fr: I) -> usize
where O: Iterator<Item=&'o mut u8>, I: Iterator<Item=&'i u8> {
    let mut count = 0;

    loop {
        match fr.next() {
            None => { return count; },
            Some(s) => { to.next().map(|t| *t = *s); count += 1; }
        }
    }
}

fn sprintf(out: &mut [u8], fmt: &[u8], args: &[&[u8]]) -> usize {
    let mut out = out.iter_mut();
    let mut fmt = fmt.iter();
    let mut arg = args.iter();
    let mut count = 0;

    loop {
        match fmt.next() {
            None => { return count; }

            Some(&b'%') => match fmt.next() {
                None => { return count; },

                Some(&b'%') => { out.next().map(|t| *t = b'%'); count += 1; },

                Some(&b's') => match arg.next() {
                    Some(arg) => count += splice(&mut out, arg.iter()),
                    None      => count += splice(&mut out, b"*".iter()),
                },

                Some(c) => {
                    out.next().map(|t| *t = b'%');
                    out.next().map(|t| *t = *c);
                    count += 2;
                },
            },

            Some(c) => { out.next().map(|t| *t = *c); count += 1; }
        }
    }
}

#[cfg(test)]
fn check_sprintf(expect: &[u8], fmt: &[u8], args: &[&[u8]]) -> bool {
    use std::mem;

    let mut buf: [u8; 2048] = unsafe { mem::uninitialized() };
    let len = sprintf(&mut buf[..], fmt, args);

    expect == &buf[..len]
}

#[test]
fn test_sprintf() {
    assert!(check_sprintf(b"hello", b"hello", &[]));
    assert!(check_sprintf(b"hello", b"he%s", &[b"llo"]));
    assert!(check_sprintf(b"hello", b"he%s%s", &[b"l", b"lo"]));
    assert!(check_sprintf(b"hello", b"%sllo", &[b"he"]));
    assert!(check_sprintf(b"%", b"%%", &[]));
}
