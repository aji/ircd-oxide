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
use irc::pluto::PlutoReader;
use irc::pluto::PlutoWriter;

pub struct Client {
    out: UnboundedSender<String>,
}

pub struct ClientError;

impl Client {
    pub fn new(out: UnboundedSender<String>) -> Client {
        Client { out: out }
    }

    pub fn handle(self, pluto: Pluto, m: Message) -> ClientOp {
        match &m.verb[..] {
            b"INC" => {
                let tx = pluto.tx(move |p| {
                    let next = p.get() + 1;
                    p.set(next);
                    self
                }).map_err(|_| ClientError);
                ClientOp::boxed(tx)
            },

            b"GET" => {
                self.out.send(format!("value is: {}", pluto.get())).unwrap();
                ClientOp::ok(self)
            },

            _ => {
                self.out.send(format!("no idea what you meant")).unwrap();
                ClientOp::ok(self)
            }
        }
    }
}

impl From<ClientError> for io::Error {
    fn from(_: ClientError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, "client error")
    }
}

pub enum ClientOp {
    Nil(Option<Result<Client, ClientError>>),
    Boxed(Box<Future<Item=Client, Error=ClientError>>)
}

impl ClientOp {
    pub fn ok(c: Client) -> ClientOp {
        ClientOp::Nil(Some(Ok(c)))
    }

    pub fn err(e: ClientError) -> ClientOp {
        ClientOp::Nil(Some(Err(e)))
    }

    pub fn boxed<F>(f: F) -> ClientOp
    where F: 'static + Future<Item=Client, Error=ClientError> {
        ClientOp::Boxed(Box::new(f))
    }
}

impl Future for ClientOp {
    type Item = Client;
    type Error = ClientError;

    fn poll(&mut self) -> Poll<Client, ClientError> {
        match *self {
            ClientOp::Nil(ref mut inner) =>
                inner.take().expect("cannot poll ClientOp twice").map(Async::Ready),

            ClientOp::Boxed(ref mut inner) =>
                inner.poll(),
        }
    }
}
