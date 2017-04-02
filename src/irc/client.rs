// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use std::io;

use futures;
use futures::Future;
use futures::Poll;
use futures::Async;
use futures::sync::mpsc::UnboundedSender;

use irc::message::Message;

pub struct Client {
    out: UnboundedSender<String>,
}

pub struct ClientError;

impl Client {
    pub fn new(out: UnboundedSender<String>) -> Client {
        Client { out: out }
    }

    pub fn handle(self, m: Message) -> ClientOp {
        println!("--> {:?}", m);
        self.out.send("neato".to_string());
        ClientOp::ok(self)
    }
}

impl From<ClientError> for io::Error {
    fn from(e: ClientError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, "client error")
    }
}

pub enum ClientOp {
    Nil(Option<Result<Client, ClientError>>)
}

impl ClientOp {
    fn ok(c: Client) -> ClientOp { ClientOp::Nil(Some(Ok(c))) }

    fn err(e: ClientError) -> ClientOp { ClientOp::Nil(Some(Err(e))) }
}

impl Future for ClientOp {
    type Item = Client;
    type Error = ClientError;

    fn poll(&mut self) -> Poll<Client, ClientError> {
        match *self {
            ClientOp::Nil(ref mut inner) =>
                inner.take().expect("cannot poll ClientOp twice").map(Async::Ready)
        }
    }
}
