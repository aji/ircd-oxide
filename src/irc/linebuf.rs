// irc/linebuf.rs -- Line buffering
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Line buffering.

/// The maximum number of bytes for each line
pub const LINE_MAX: usize = 2048;

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
    /// with each slice. If the callback returns `false`, processing is stopped
    /// and `split` returns with the value, copying the rest of the data into
    /// its internal buffer to be processed later.
    pub fn split<F>(&mut self, data: &[u8], mut cb: F)
    where F: FnMut(&[u8]) -> bool {
        assert!(data.len() < LINE_MAX);

        let bytes = {
            let mut bytes = self.extra.clone();
            bytes.extend(data.iter().cloned().filter(|c| *c != b'\r'));
            bytes
        };

        let mut buf = &bytes[..];

        while buf.len() > 0 {
            let i = match buf.iter().position(|c| *c == b'\n') {
                Some(i) => i,
                None => break
            };

            let cont = cb(&buf[..i]);
            buf = &buf[i+1..];

            if !cont {
                break;
            }
        }

        self.extra = buf.iter().cloned().collect();
    }
}

#[test]
fn easy_split() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    lb.split(
        b"line1\nline2\n" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); true }
    );

    assert!(&lines[0][..] == b"line1");
    assert!(&lines[1][..] == b"line2");
}

#[test]
fn harder_split() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    lb.split(
        b"line" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); true }
    );
    lb.split(
        b"1\nline2" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); true }
    );
    lb.split(
        b"\n" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); true }
    );

    assert!(&lines[0][..] == b"line1");
    assert!(&lines[1][..] == b"line2");
}

#[test]
fn caller_stops() {
    let mut lb = LineBuffer::new();
    let mut lines: Vec<Vec<u8>> = Vec::new();

    lb.split(b"line1\nline2\n" as &[u8], |_| false);

    lb.split(
        b"" as &[u8],
        |ln| { lines.push(ln.iter().cloned().collect()); true }
    );

    assert!(&lines[0][..] == b"line2");
}
