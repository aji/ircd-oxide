// irc/post.rs -- the post office, a messaging abstraction
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! An IRC messaging abstraction

use mio::tcp::TcpStream;
use rand::random;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::mem;

use irc::LineBuffer;

/// A token to identify a stream in the `PostOffice`.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct PostToken(u32);

/// The main post office struct.
///
/// This will take ownership of all your mio `TcpStream`s and give you a token
/// which you can then use to send and receive messages through an immutable
/// reference.
pub struct PostOffice {
    boxes: HashMap<PostToken, PostBox>
}

struct PostBox {
    lb: RefCell<LineBuffer>,
    sock: RefCell<TcpStream>
}

impl PostOffice {
    /// Creates an empty post office
    pub fn new() -> PostOffice {
        PostOffice { boxes: HashMap::new() }
    }

    /// Adopts a `TcpStream`
    pub fn insert(&mut self, sock: TcpStream) -> PostToken {
        let tok = PostToken(random());
        self.boxes.insert(tok, PostBox::new(sock));
        tok
    }

    /// Drops a `TcpStream`
    pub fn remove(&mut self, sock: &PostToken) {
        self.boxes.remove(sock);
    }

    /// Reads lines from a named stream, with the same API as
    /// `LineBuffer::split`
    pub fn read<F, T>(&self, sock: &PostToken, cb: F) -> io::Result<Option<T>>
    where F: FnMut(&[u8]) -> Option<T> {
        match self.boxes.get(sock) {
            Some(b) => b.read(cb),
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                "invalid socket token"
            )),
        }
    }

    /// Writes a buffer to the named stream.
    pub fn write(&self, sock: &PostToken, data: &[u8]) -> io::Result<usize> {
        match self.boxes.get(sock) {
            Some(b) => b.write(data),
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                "invalid socket token"
            )),
        }
    }
}

impl PostBox {
    fn new(sock: TcpStream) -> PostBox {
        PostBox {
            lb: RefCell::new(LineBuffer::new()),
            sock: RefCell::new(sock)
        }
    }

    fn read<F, T>(&self, cb: F) -> io::Result<Option<T>>
    where F: FnMut(&[u8]) -> Option<T> {
        let mut buf: [u8; 2048] = unsafe { mem::uninitialized() };

        let len = {
            // I don't *really* have to do it this way, but I want to be sure
            // that the guard is dropped.
            let mut guard = self.sock.borrow_mut();
            let len = try!(guard.read(&mut buf));
            drop(guard);
            len
        };

        Ok(self.lb.borrow_mut().split(&buf[..len], cb))
    }

    fn write(&self, data: &[u8]) -> io::Result<usize> {
        self.sock.borrow_mut().write(data)
    }
}
