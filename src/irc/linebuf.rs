// irc/linebuf.rs -- Line buffering
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Line buffering.
//!
//! *TODO*: Implement/enforce limit on line lengths.

/// A line buffer.
pub struct LineBuffer {
    extra: Vec<u8>,
}

impl LineBuffer {
    /// Creates an empty `LineBuffer`.
    pub fn new() -> LineBuffer {
        LineBuffer { extra: Vec::new() }
    }

    /// Splits some incoming data into lines and calls the callback function
    /// with each slice. If the callback returns `Some`, processing is stopped
    /// and `split` returns with the value, copying the rest of the data into
    /// its internal buffer to be processed later.
    pub fn split<F, T>(&mut self, data: &[u8], mut cb: F) -> Option<T>
    where F: FnMut(&[u8]) -> Option<T> {
        let bytes = {
            let mut bytes = self.extra.clone();
            bytes.extend(data.iter().cloned().filter(|c| *c != b'\r'));
            bytes
        };

        let mut buf = &bytes[..];
        let mut cont = None;

        while buf.len() > 0 && cont.is_none() {
            let i = match buf.iter().position(|c| *c == b'\n') {
                Some(i) => i,
                None => break
            };

            cont = cb(&buf[..i]);
            buf = &buf[i+1..];
        }

        self.extra = buf.iter().cloned().collect();

        cont
    }
}

#[test]
fn easy_split() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    let a: Option<()> = lb.split(
        b"line1\nline2\n" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); None }
    );

    assert!(a.is_none());
    assert!(&lines[0][..] == b"line1");
    assert!(&lines[1][..] == b"line2");
}

#[test]
fn harder_split() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    let a: Option<()> = lb.split(
        b"line" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); None }
    );
    let b: Option<()> = lb.split(
        b"1\nline2" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); None }
    );
    let c: Option<()> = lb.split(
        b"\n" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); None }
    );

    assert!(a.is_none());
    assert!(b.is_none());
    assert!(c.is_none());
    assert!(&lines[0][..] == b"line1");
    assert!(&lines[1][..] == b"line2");
}

#[test]
fn caller_stops() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    let a: Option<()> = lb.split(b"line1\nline2\n" as &[u8], |_| Some(()));

    let b: Option<()> = lb.split(
        b"" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); None }
    );

    assert!(a.is_some());
    assert!(b.is_none());
    assert!(&lines[0][..] == b"line2");
}
