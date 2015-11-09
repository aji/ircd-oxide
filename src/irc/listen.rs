// listen.rs -- listeners
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Listeners

use std::convert::From;
use std::io;
use mio;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;

/// A listener
pub struct Listener {
    sock: TcpListener
}

impl Listener {
    /// Wraps a mio `TcpListener` as a `Listener`
    pub fn new(sock: TcpListener) -> Listener {
        Listener {
            sock: sock
        }
    }

    /// Registers the `Listener` with the given mio `EventLoop`
    pub fn register<H>(&self, tok: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        ev.register_opt(
            &self.sock,
            tok,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        )
    }

    /// Accepts a new connection.
    pub fn accept<S>(&mut self) -> io::Result<S>
    where S: From<TcpStream> {
        let sock = {
            let sock = try!(self.sock.accept());
            // TODO: don't expect, maybe?
            sock.expect("accept failed (would block")
        };

        Ok(S::from(sock))
    }
}
