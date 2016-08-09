// event.rs -- event loop
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Event loop: ergonomic `mio` wrapper.

use mio;
use rand::random;
use std::collections::HashMap;
use std::io;

/// Type alias for the `mio` event loop
pub type LooperLoop<X> = mio::EventLoop<Looper<X>>;

/// Type alias for the return type of `add` functions
pub type NewPollable<X> = io::Result<Box<Pollable<X>>>;

/// `Looper` is the core of this event loop API. It owns a user-defined context and a
/// family of pollables named with `mio::Token`s.
pub struct Looper<X: Context> {
    pollables: HashMap<mio::Token, Box<Pollable<X>>>,
    context: X
}

impl<X: Context> Looper<X> {
    /// Creates a new `Looper` using the given context
    pub fn new(ctx: X) -> Looper<X> {
        Looper {
            pollables: HashMap::new(),
            context: ctx,
        }
    }

    /// Adds a pollable to this `Looper`. The function is called with the `mio` event loop
    /// and the generated `mio` token. The function, in turn, returns the pollable to be
    /// associated with the token. The function should also ensure that the pollable is correctly
    /// registered with the event loop.
    pub fn add<F>(&mut self, ev: &mut LooperLoop<X>, f: F) -> io::Result<()>
    where F: Fn(&mut LooperLoop<X>, mio::Token) -> NewPollable<X> {
        let token = mio::Token(random());
        let p = try!(f(ev, token));
        self.pollables.insert(token, p);
        Ok(())
    }

    /// Sends a message to the named pollable.
    pub fn signal(&mut self, tk: mio::Token, msg: X::Message) {
        match self.pollables.get_mut(&tk) {
            Some(p) => p.message(&mut self.context, msg),
            None => warn!("got signal for token we don't know about: {:?}", tk),
        }
    }
}

impl<X: Context> mio::Handler for Looper<X> {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, ev: &mut LooperLoop<X>, tk: mio::Token, _: mio::EventSet) {
        let mut actions = LooperActions::new(self);

        match self.pollables.get_mut(&tk) {
            Some(p) => {
                if let Err(e) = p.ready(&mut self.context, &mut actions) {
                    error!("dropping pollable {:?}: {}", tk, e);
                    actions.drop(tk);
                }
            },

            None => {
                error!("mio woke us up with token we don't know about: {:?}", tk);
                return;
            },
        }

        actions.apply(self, ev, tk);
    }
}

/// Pollables can post actions on a `Looper` through a `LooperActions`. Due to Rust's
/// rules on borrowing (which I've come to realize is a good thing in this case) it's not
/// possible for a pollable (owned by `Looper`) to have a mutable reference to itself and
/// also a mutable reference to the `Looper` that owns it while handling an event. Therefore,
/// a `LooperActions` is passed to the pollable instead. `LooperActions` stores the actions
/// the pollable wanted to perform and then applies them when the pollable is finished
/// handling the event (i.e. when the mutable borrow of the pollable ends). A significant
/// consequence is that, while the code may read like actions are being performed then and
/// there in the pollable handler, they're actually being deferred.
pub struct LooperActions<X: Context> {
    to_drop: Vec<mio::Token>,
    to_add: Vec<Box<Fn(&mut LooperLoop<X>, mio::Token) -> NewPollable<X>>>,
    messages: Vec<(mio::Token, X::Message)>,
}

impl<X: Context> LooperActions<X> {
    fn new(_: &mut Looper<X>) -> LooperActions<X> {
        LooperActions {
            to_drop: Vec::new(),
            to_add: Vec::new(),
            messages: Vec::new(),
        }
    }

    fn apply(self, looper: &mut Looper<X>, ev: &mut LooperLoop<X>, tk: mio::Token) {
        // TODO: drop

        for f in self.to_add {
            looper.add(ev, |c, t| f(c, t));
        }

        for (tk, m) in self.messages {
            looper.signal(tk, m);
        }
    }

    /// Requests that the given pollable be dropped.
    pub fn drop(&mut self, tk: mio::Token) {
        self.to_drop.push(tk);
    }

    /// Requests an add to be performed when the pollable returns.
    pub fn add<F: 'static>(&mut self, f: F)
    where F: Fn(&mut LooperLoop<X>, mio::Token) -> NewPollable<X> {
        self.to_add.push(Box::new(f));
    }

    /// Requests the given pollable be signaled.
    pub fn signal(&mut self, tk: mio::Token, msg: X::Message) {
        self.messages.push((tk, msg));
    }
}

/// A trait that all `Looper` user data must implement.
pub trait Context {
    /// The type of message that pollables can send to each other.
    type Message;
}

/// A trait that all `Looper`-capable pollables must implement.
pub trait Pollable<X: Context> {
    /// Called when an event is ready that the pollable has requested
    fn ready(&mut self, ctx: &mut X, act: &mut LooperActions<X>) -> io::Result<()>;

    /// Called to deliver a message to this pollable. The message format is defined by
    /// the context.
    fn message(&mut self, ctx: &mut X, msg: X::Message) { }
}

/// A function to run a simple event loop
pub fn run<X: Context, F>(ctx: X, init: F) -> io::Result<()>
where F: Fn(&mut Looper<X>, &mut LooperLoop<X>) -> io::Result<()> {
    let mut looper = Looper::new(ctx);
    let mut ev = try!(mio::EventLoop::new());

    try!(init(&mut looper, &mut ev));

    ev.run(&mut looper)
}
