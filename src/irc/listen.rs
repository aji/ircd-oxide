// listen.rs -- listeners
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Listeners

use mio;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;
use std::io;
use std::net::ToSocketAddrs;

use irc::client::Client;
use looper::LooperActions;
use looper::LooperLoop;
use looper::Pollable;
use run::Top;

/// A listener
pub struct Listener {
    name: mio::Token,
    sock: TcpListener
}

impl Listener {
    /// Wraps a mio `TcpListener` as a `Listener`
    pub fn new<A: ToSocketAddrs>(addr: A, ev: &mut LooperLoop<Top>, name: mio::Token)
    -> io::Result<Listener> {
        let sock = {
            let mut addrs = try!(ToSocketAddrs::to_socket_addrs(&addr));
            let addr = match addrs.nth(0) {
                Some(addr) => addr,
                None => panic!("help!"),
            };
            debug!("listening on {:?}", addr);
            try!(mio::tcp::TcpListener::bind(&addr))
        };

        try!(ev.register(&sock, name, mio::EventSet::readable(), mio::PollOpt::level()));

        Ok(Listener { name: name, sock: sock })
    }
}

impl Pollable<Top> for Listener {
    fn ready(&mut self, ctx: &mut Top, act: &mut LooperActions<Top>) -> io::Result<()> {
        let sock = {
            let sock = try!(self.sock.accept());
            // TODO: don't expect, maybe?
            sock.expect("accept failed (would block)").0
        };

        act.add(move |ev, tk| {
            match Client::new(sock, ev, tk) {
                Ok(c) => Ok(Box::new(c)),
                Err(e) => Err(e),
            }
        });

        Ok(())
    }
}
