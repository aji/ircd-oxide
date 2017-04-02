// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use std::io;

use futures;
use futures::Future;
use futures::IntoFuture;
use futures::Poll;
use futures::Async;
use futures::sync::mpsc::UnboundedSender;

use irc::message::Message;
use irc::pluto::Pluto;
use irc::pluto::PlutoTx;

pub struct Client {
    out: UnboundedSender<String>,
}

pub struct ClientError;

impl Client {
    pub fn new(out: UnboundedSender<String>) -> Client {
        Client { out: out }
    }

    pub fn handle(self, pluto: Pluto, m: Message) -> Box<Future<Item=Client, Error=ClientError>> {
        let tx = pluto.tx(move |p| {
            if m.verb == "INC" {
                let next = p.get() + 1;
                p.set(next);
            } else {
                self.out.send(format!("no idea what you meant"));
            }
            self
        }).map_err(|e| {
            ClientError
        });

        Box::new(tx)
    }
}

impl From<ClientError> for io::Error {
    fn from(e: ClientError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, "client error")
    }
}

/*
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
*/
