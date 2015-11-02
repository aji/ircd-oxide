// irc/client.rs -- client protocol handlers
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client protocol handlers

use mio;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

use state::world;
use state::Channel;
use state::Diffable;
use state::Differ;
use state::Id;
use state::World;

/// A pool of clients
pub struct ClientPool;

/// The structure that holds a pool of clients and responds to events
#[derive(Clone)]
pub struct ClientManager {
    pool: Rc<RefCell<ClientPool>>,
}

impl ClientPool {
    fn new() -> ClientPool {
        ClientPool
    }

    fn channels_changed(
        &mut self,
        old: &HashMap<Id<Channel>, Channel>,
        new: &HashMap<Id<Channel>, Channel>
    ) {
        for diff in old.diff(new) {
            match diff {
                Differ::Added((id, chan)) => {
                    self.channel_added(id, chan);
                },

                Differ::Removed((id, chan)) => {
                    self.channel_removed(id, chan);
                },

                Differ::Changed((id, chan_old), (_, chan_new)) => {
                    self.channel_changed(id, chan_old, chan_new);
                },
            }
        }
    }

    fn channel_added(&mut self, id: &Id<Channel>, chan: &Channel) {
        println!("channel added");
    }

    fn channel_removed(&mut self, id: &Id<Channel>, chan: &Channel) {
        println!("channel removed");
    }

    fn channel_changed(
        &mut self,
        id: &Id<Channel>,
        chan_old: &Channel,
        chan_new: &Channel
    ) {
        println!("channel changed");

        let topic_diff = chan_old.topic.diff(&chan_new.topic);
        if let Some(Differ::Changed(_, topic)) = topic_diff {
            println!("new topic: {}", *topic);
        }
    }
}

impl ClientManager {
    /// Creates a new `ClientManager` with an empty `ClientPool`
    pub fn new() -> ClientManager {
        ClientManager {
            pool: Rc::new(RefCell::new(ClientPool::new())),
        }
    }
}

impl world::Observer for ClientManager {
    fn world_changed(&mut self, old: &World, new: &World) {
        self.pool.borrow_mut().channels_changed(old.channels(), new.channels());
    }
}

/// A listener that can spawn new TCP connections
pub struct Listener {
    sock: TcpListener,
}

impl Listener {
    /// Wraps the mio `TcpListener` as a `Listener`
    pub fn new(sock: TcpListener) -> Listener {
        Listener { sock: sock }
    }

    /// Registers the `Listener` with the given mio `EventLoop`
    pub fn register<H>(&self, tk: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        ev.register_opt(
            &self.sock,
            tk,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        )
    }

    /// Accepts a new connection
    pub fn accept(&mut self) -> io::Result<PendingClient> {
        let sock = {
            let sock = try!(self.sock.accept());
            sock.expect("accept failed (would block)")
        };

        Ok(PendingClient::new(sock))
    }
}

/// A client that has connected but not finished registration
pub struct PendingClient {
    sock: TcpStream,
}

impl PendingClient {
    /// Wraps the mio `TcpStream` as a `PendingClient`
    pub fn new(sock: TcpStream) -> PendingClient {
        PendingClient { sock: sock }
    }

    /// Registers the `PendingClient` with the given mio `EventLoop`
    pub fn register<H>(&self, tk: mio::Token, ev: &mut mio::EventLoop<H>)
    -> io::Result<()> where H: mio::Handler {
        ev.register_opt(
            &self.sock,
            tk,
            mio::EventSet::readable(),
            mio::PollOpt::level()
        )
    }
}
