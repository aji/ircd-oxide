// irc/pending.rs -- Pending client handlers
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Pending client handlers

use irc::CommandSet;
use irc::Message;

/// Pending client data
pub struct PendingClient;

/// A pending client handler.
pub struct PendingHandler<'c> {
    cmds: CommandSet<(&'c mut PendingClient, Message<'c>), ()>
}

impl<'c> PendingHandler<'c> {
    /// Creates a new pending client handling structure.
    pub fn new() -> PendingHandler<'c> {
        PendingHandler {
            cmds: CommandSet::new()
        }
    }

    /// Handles a message from a pending client.
    pub fn handle(&self, ctx: &'c mut PendingClient, m: Message<'c>) {
        self.cmds.handle(m.verb, (ctx, m));
    }
}
