// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use irc::global::IRCD;
use irc::CommandSet;
use irc::Message;

/// The context of an incoming client message.
pub struct ClientContext;

/// A client handler.
pub struct ClientHandler<'c> {
    cmds: CommandSet<(&'c mut IRCD, ClientContext, Message<'c>), ()>
}

impl<'c> ClientHandler<'c> {
    /// Creates a new client handling structure.
    pub fn new() -> ClientHandler<'c> {
        ClientHandler {
            cmds: CommandSet::new()
        }
    }

    /// Handles a message from a client.
    pub fn handle(&self, ircd: &'c mut IRCD, ctx: ClientContext, m: Message<'c>) {
        self.cmds.handle(m.verb, (ircd, ctx, m));
    }
}
