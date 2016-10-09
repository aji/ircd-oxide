// irc/client.rs -- client handling
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client handling

use mio;
use mio::tcp::TcpStream;
use std::collections::HashMap;
use std::io;
use take_mut;

use irc::global::IRCD;
use irc::message::Message;
use irc::net::IrcStream;
use irc::output::IrcFormatter;
use looper::LooperActions;
use looper::LooperLoop;
use looper::Pollable;
use state::id::Id;
use state::identity::Identity;
use state::world::WorldView;
use top;

/// An IRC client
pub struct Client {
    name: mio::Token,
    fmt: IrcFormatter,
    sock: IrcStream,
    state: ClientState,
}

impl Client {
    /// Wraps an `TcpStream` as a `Client`
    pub fn new(ctx: &mut top::Context, sock: TcpStream, ev: &mut LooperLoop, name: mio::Token)
    -> io::Result<Client> {
        let ircsock = IrcStream::new(sock);
        try!(ircsock.register(name, ev));
        Ok(Client {
            name: name,
            fmt: ctx.ircd.formatter(),
            sock: ircsock,
            state: ClientState::start()
        })
    }
}

impl Pollable for Client {
    /// Called to indicate data is ready on the client's socket.
    fn ready(&mut self, ctx: &mut top::Guard, act: &mut LooperActions) -> io::Result<()> {
        let fmt = &self.fmt;
        let sock = &self.sock;
        let state = &mut self.state;

        if self.sock.empty() {
            act.drop(self.name);
            return Ok(());
        }

        let _: Option<()> = try!(self.sock.read(|ln| {
            let m = match Message::parse(ln) {
                Ok(m) => m,
                Err(_) => return None,
            };

            debug!("--> {}", String::from_utf8_lossy(ln));
            debug!("    {:?}", m);

            state.handle(ctx, &m, fmt, sock);

            None
        }));

        Ok(())
    }
}

enum ClientState {
    Pending(PendingData),
    Active(ActiveData),
}

impl ClientState {
    fn start() -> ClientState { ClientState::Pending(PendingData::new()) }

    fn handle(
        &mut self,
        ctx: &mut top::Guard,
        m: &Message,
        fmt: &IrcFormatter,
        sock: &IrcStream,
    ) {
        match *self {
            ClientState::Pending(ref mut data) => data.handle_pending(ctx, m, fmt, sock),
            ClientState::Active(ref mut data) => data.handle_active(ctx, m, fmt, sock),
        }

        take_mut::take(self, |state| {
            match state {
                ClientState::Pending(data) => data.try_promote(ctx, fmt, sock),
                other => other,
            }
        });
    }
}

struct PendingData {
    password: Option<String>,
    nickname: Option<String>,
    username: Option<String>,
    realname: Option<String>,
}

impl PendingData {
    fn new() -> PendingData {
        PendingData {
            password: None,
            nickname: None,
            username: None,
            realname: None,
        }
    }

    fn handle_pending(
        &mut self,
        _ctx: &mut top::Guard,
        m: &Message,
        _fmt: &IrcFormatter,
        _sock: &IrcStream
    ) {
        match m.verb {
            "PASS" => self.password = Some(m.args[0].to_string()),

            "NICK" => self.nickname = Some(m.args[0].to_string()),

            "USER" => {
                self.username = Some(m.args[0].to_string());
                self.realname = Some(m.args[3].to_string());
            },

            _ => { }
        }
    }

    fn try_promote(self, ctx: &mut top::Guard, fmt: &IrcFormatter, sock: &IrcStream)
    -> ClientState {
        let promotion = match Promotion::from_pending(self) {
            Ok(promotion) => promotion,
            Err(data) => {
                // user is missing something
                return ClientState::Pending(data);
            }
        };

        // TODO: actually process the promotion
        let _ = promotion.password;
        let _ = promotion.nickname;
        let _ = promotion.username;
        let _ = promotion.realname;

        let identity = ctx.world.create_temp_identity();

        // unused_must_use can be cleaned up when try_promote is able to return something other
        // than ClientState, or when the actual output part is moved away from this function.
        rpl_welcome!(fmt, sock);
        rpl_isupport!(fmt, sock, "CHANTYPES=#");

        ClientState::Active(ActiveData {
            identity: identity,
        })
    }
}

struct ActiveData {
    identity: Id<Identity>,
}

impl ActiveData {
    fn handle_active(
        &mut self,
        ctx: &mut top::Guard,
        m: &Message,
        _fmt: &IrcFormatter,
        _sock: &IrcStream
    ) {
        match m.verb {
            "JOIN" => {
                let chname = m.args[0].to_string();

                let chan = match ctx.world.channel_name_owner(&chname).cloned() {
                    Some(chan) => chan,
                    None => {
                        let chan = ctx.world.create_channel();
                        ctx.world.channel_claim(chan.clone(), chname.clone());
                        chan
                    }
                };

                ctx.world.channel_user_add(chan, self.identity.clone());
            },

            _ => { }
        }
    }
}

struct Promotion {
    password: String,
    nickname: String,
    username: String,
    realname: String,
}

impl Promotion {
    fn from_pending(data: PendingData) -> Result<Promotion, PendingData> {
        if data.password.is_none() || data.nickname.is_none() ||
                data.username.is_none() || data.realname.is_none() {
            Err(data)
        } else {
            Ok(Promotion {
                password: data.password.unwrap(),
                nickname: data.nickname.unwrap(),
                username: data.username.unwrap(),
                realname: data.realname.unwrap(),
            })
        }
    }
}
