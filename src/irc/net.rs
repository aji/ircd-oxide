// irc/net.rs -- an IRC messaging abstraction
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! An IRC messaging abstraction

// I'm a little disappointed that this module exists. It uses RefCells, which
// can panic if usage patterns aren't correct. With the design of this API, this
// can only happen if two calls are made to `read` at the same time, but it's
// still a code smell, and I don't like it at all.

use mio;
use mio::tcp::TcpStream;
use std::cell::Cell;
use std::cell::RefCell;
use std::io;
use std::io::prelude::*;
use std::mem;

use irc::LineBuffer;

/// An IRC stream that can be interacted with through immutable references.
pub struct IrcStream {
    empty: Cell<bool>,
    lb: RefCell<LineBuffer>,
    sock: RefCell<TcpStream>
}

impl IrcStream {
    /// Creates a new IRC stream from a mio `TcpStream`
    pub fn new(sock: TcpStream) -> IrcStream {
        IrcStream {
            empty: Cell::new(false),
            lb: RefCell::new(LineBuffer::new()),
            sock: RefCell::new(sock)
        }
    }

    /// Registers the `IrcStream` with the given `EventLoop`
    pub fn register<H>(&self, tok: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        ev.register_opt(
            &*self.sock.borrow(),
            tok,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        )
    }

    /// Returns true if the `IrcStream` is empty
    pub fn empty(&self) -> bool {
        self.empty.get()
    }

    /// Reads some lines from the stream, using the same API as `LineBuffer`
    pub fn read<F, T>(&self, cb: F) -> io::Result<Option<T>>
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

        if len == 0 {
            self.empty.set(true);
            Ok(None)
        } else {
            Ok(self.lb.borrow_mut().split(&buf[..len], cb))
        }
    }

    /// Writes some data to the stream. This is more or less a proxy for the
    /// `Write::write` implementation on the underlying `TcpStream`
    pub fn write(&self, data: &[u8]) -> io::Result<usize> {
        self.sock.borrow_mut().write(data)
    }
}
