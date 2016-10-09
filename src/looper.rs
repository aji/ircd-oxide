// event.rs -- event loop
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Event loop: ergonomic `mio` wrapper.

// This API can be genericized when lifetime parameters are supported with associated types.
// I had wanted a trait that had something like this:
//
//     trait Context {
//         type Guard<'g>;
//         fn start<'a: 'g>(&'a mut self) -> Guard<'g>;
//     }
//
// After wrestling the compiler for too long, I decided it wasn't possible (not easily anyway)
// and un-genericized this API. It wouldn't be too much work to adapt Looper to be more generic,
// if you don't need some kind of event-scoped guard type.

use mio;
use rand::random;
use std::boxed::FnBox;
use std::collections::HashMap;
use std::io;

use top;

/// Type alias for the `mio` event loop
pub type LooperLoop = mio::EventLoop<Looper>;

/// Type alias for the return type of `add` functions
pub type NewPollable = io::Result<Box<Pollable>>;

/// `Looper` is the core of this event loop API. It owns a user-defined context and a
/// family of pollables named with `mio::Token`s.
pub struct Looper {
    pollables: HashMap<mio::Token, Box<Pollable>>,
    context: top::Context
}

impl Looper {
    /// Creates a new `Looper` using the given context
    pub fn new(ctx: top::Context) -> Looper {
        Looper {
            pollables: HashMap::new(),
            context: ctx,
        }
    }

    /// Drops the named pollable from the event loop
    pub fn drop(&mut self, ev: &mut LooperLoop, tk: mio::Token) -> io::Result<()> {
        match self.pollables.remove(&tk) {
            Some(p) => p.deregister(ev),
            None => Ok(())
        }
    }

    /// Adds a pollable to this `Looper`. The function is called with the `mio` event loop
    /// and the generated `mio` token. The function, in turn, returns the pollable to be
    /// associated with the token. The function should also ensure that the pollable is correctly
    /// registered with the event loop.
    pub fn add<F>(&mut self, ev: &mut LooperLoop, f: F) -> io::Result<()>
    where F: FnOnce(&mut top::Context, &mut LooperLoop, mio::Token) -> NewPollable {
        let token = mio::Token(random());
        let p = try!(f(&mut self.context, ev, token));
        self.pollables.insert(token, p);
        Ok(())
    }

    /// Sends a message to the named pollable.
    pub fn signal(&mut self, tk: mio::Token, msg: top::Message) {
        match self.pollables.get_mut(&tk) {
            Some(p) => p.message(&mut self.context, msg),
            None => warn!("got signal for token we don't know about: {:?}", tk),
        }
    }
}

impl mio::Handler for Looper {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, ev: &mut LooperLoop, tk: mio::Token, _: mio::EventSet) {
        let mut actions = LooperActions::new(self);

        match self.pollables.get_mut(&tk) {
            Some(p) => {
                self.context.on_event(&mut actions, |guard, act| {
                    if let Err(e) = p.ready(guard, act) {
                        error!("dropping pollable {:?}: {}", tk, e);
                        act.drop(tk);
                    }
                });
            },

            None => {
                error!("mio woke us up with token we don't know about: {:?}", tk);
                return;
            },
        }

        actions.apply(self, ev, tk);
    }
}

/// Pollables can post actions on a `Looper` through a `LooperActions`
///
/// Due to Rust's rules on borrowing (which I've come to realize is a good thing in this case)
/// it's not possible for a pollable (owned by `Looper`) to have a mutable reference to itself
/// and also a mutable reference to the `Looper` that owns it while handling an event. Therefore,
/// a `LooperActions` is passed to the pollable instead. `LooperActions` stores the actions
/// the pollable wanted to perform and then applies them when the pollable is finished handling
/// the event (i.e. when the mutable borrow of the pollable ends). A significant consequence is
/// that, while the code may read like actions are being performed then and there in the pollable
/// handler, they're actually being deferred.
pub struct LooperActions {
    to_drop: Vec<mio::Token>,
    to_add: Vec<Box<FnBox(&mut top::Context, &mut LooperLoop, mio::Token) -> NewPollable>>,
    messages: Vec<(mio::Token, top::Message)>,
}

impl LooperActions {
    fn new(_: &mut Looper) -> LooperActions {
        LooperActions {
            to_drop: Vec::new(),
            to_add: Vec::new(),
            messages: Vec::new(),
        }
    }

    fn apply(self, looper: &mut Looper, ev: &mut LooperLoop, _tk: mio::Token) {
        // TODO: figure out what to do with errors here

        for tk in self.to_drop.into_iter() {
            let _ = looper.drop(ev, tk);
        }

        for f in self.to_add.into_iter() {
            let _ = looper.add(ev, |x, c, t| f.call_box((x, c, t)));
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
    where F: FnOnce(&mut top::Context, &mut LooperLoop, mio::Token) -> NewPollable {
        self.to_add.push(Box::new(f));
    }

    /// Requests the given pollable be signaled.
    pub fn signal(&mut self, tk: mio::Token, msg: top::Message) {
        self.messages.push((tk, msg));
    }
}

/// A trait that all `Looper`-capable pollables must implement.
pub trait Pollable {
    /// Called when an event is ready that the pollable has requested
    fn ready(&mut self, ctx: &mut top::Guard, act: &mut LooperActions) -> io::Result<()>;

    /// Called to deliver a message to this pollable. The message format is defined by
    /// the context.
    fn message(&mut self, _ctx: &mut top::Context, _msg: top::Message) { }

    /// Called when the pollable should remove itself from the event loop
    fn deregister(&self, ev: &mut LooperLoop) -> io::Result<()>;
}

/// A function to run a simple event loop
pub fn run<F>(ctx: top::Context, init: F) -> io::Result<()>
where F: Fn(&mut Looper, &mut LooperLoop) -> io::Result<()> {
    let mut looper = Looper::new(ctx);
    let mut ev = try!(mio::EventLoop::new());

    try!(init(&mut looper, &mut ev));

    ev.run(&mut looper)
}
