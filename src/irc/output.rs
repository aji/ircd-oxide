// irc/output.rs -- a module for formatting IRC messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

// TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO
// TODO   THIS IS BAD AND PROBABLY NEEDS TO BE FIXED SOMEHOW   TODO
// TODO   THIS IS BAD AND PROBABLY NEEDS TO BE FIXED SOMEHOW   TODO
// TODO   THIS IS BAD AND PROBABLY NEEDS TO BE FIXED SOMEHOW   TODO
// TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO

//! Formatting IRC messages

use std::io;
use std::fmt;

use irc::net::IrcStream;

/// A formatter for IRC lines
pub struct IrcFormatter {
    server: String,
    nick: Option<String>,
}

impl IrcFormatter {
    /// Creates a new formatter using the given server name for
    /// server-originated messages.
    pub fn new(server: &str) -> IrcFormatter {
        IrcFormatter { server: server.to_string(), nick: None }
    }

    fn nick_str(&self) -> &str {
        match self.nick.as_ref() {
            Some(n) => n,
            None => "*",
        }
    }

    /// Sends a numeric to the client
    pub fn numeric(&self, sock: &IrcStream, num: u32, msg: fmt::Arguments) -> io::Result<()> {
        self.send_all(sock, format!(":{} {:03} {} {}\r\n", self.server, num, self.nick_str(), msg))
    }

    /// Sends a notice to the client, from the server
    pub fn snotice(&self, sock: &IrcStream, msg: fmt::Arguments) -> io::Result<()> {
        self.send_all(sock, format!(":{} NOTICE {} :{}\r\n", self.server, self.nick_str(), msg))
    }

    fn send_all(&self, sock: &IrcStream, msg: String) -> io::Result<()> {
        let mut out = msg.as_bytes();
        while out.len() > 0 {
            let len = try!(sock.write(out));
            out = &out[len..];
        }
        Ok(())
    }
}
