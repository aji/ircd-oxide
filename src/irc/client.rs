// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use std::collections::HashMap;

use irc::global::IRCD;
use irc::net::IrcStream;
use irc::Message;

// Simplifies comand invocations
struct ClientContext<'c> {
    ircd: &'c IRCD,
    sock: &'c IrcStream,
}

// make sure to keep this in sync with the constraint on `ClientHandler::add`.
struct HandlerFn {
    args: usize,
    cb: Box<for<'c> Fn(&mut ClientContext<'c>, &Message<'c>)>,
}

/// A client handler.
pub struct ClientHandler {
    handlers: HashMap<Vec<u8>, HandlerFn>,
}

impl ClientHandler {
    /// Creates a new client handling structure.
    pub fn new() -> ClientHandler {
        let mut ch = ClientHandler {
            handlers: HashMap::new(),
        };

        handlers(&mut ch);

        ch
    }

    /// Adds a handler function. If a handler is already defined for the given
    /// verb, nothing is added.
    fn add<F>(&mut self, verb: &[u8], args: usize, func: F)
    where F: 'static + for<'c> Fn(&mut ClientContext<'c>, &Message<'c>) {
        self.handlers.entry(verb.to_vec()).or_insert_with(|| HandlerFn {
            args: args,
            cb: Box::new(func)
        });
    }

    /// Handles a message from a client.
    fn handle<'c>(&self, ctx: &mut ClientContext<'c>, m: &Message<'c>) {
        match self.handlers.get(m.verb) {
            Some(hdlr) => {
                if m.args.len() < hdlr.args {
                    debug!("not enough args!");
                } else {
                    (hdlr.cb)(ctx, m);
                }
            },

            None => {
                debug!("client used unknown command");
            }
        }
    }
}

// in a funtion so we can dedent
fn handlers(_ch: &mut ClientHandler) {
}
