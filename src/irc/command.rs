// irc/command.rs -- command handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Command handling framework.

use std::collections::HashMap;

/// A set of command handlers
pub struct CommandSet<X, Y> {
    cmds: HashMap<Vec<u8>, Box<Fn(X) -> Y>>
}

impl<X, Y> CommandSet<X, Y> {
    /// Creates an empty `CommandSet`
    pub fn new() -> CommandSet<X, Y> {
        CommandSet {
            cmds: HashMap::new()
        }
    }

    /// Adds a new command handler
    pub fn cmd<F>(&mut self, verb: &[u8], f: F)
    where F: 'static + Fn(X) -> Y {
        self.cmds.insert(verb.to_vec(), Box::new(f));
    }

    /// Handles a command
    pub fn handle(&self, verb: &[u8], x: X) -> Option<Y> {
        self.cmds.get(verb).map(|h| h(x))
    }
}
