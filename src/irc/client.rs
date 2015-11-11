// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

/// The context of an incoming client message.
pub struct ClientContext;

/// A client handler.
pub struct ClientHandler;

impl ClientHandler {
    /// Creates a new client handling structure.
    pub fn new() -> ClientHandler {
        ClientHandler
    }
}
