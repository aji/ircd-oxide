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
use irc::numeric::*;
use irc::output::IrcFormatter;
use looper::LooperActions;
use looper::LooperLoop;
use looper::Pollable;
use run::Top;
use state::id::Id;
use state::identity::Identity;
use state::world::WorldView;

fn to_string(v: &[u8]) -> Option<String> {
    String::from_utf8(v.to_vec()).ok()
}

/// An IRC client
pub struct Client {
    name: mio::Token,
    fmt: IrcFormatter,
    sock: IrcStream,
    state: ClientState,
}

enum ClientState {
    Pending(PendingData),
    Active(ActiveData),
}

impl Client {
    /// Wraps an `TcpStream` as a `Client`
    pub fn new(ctx: &mut Top, sock: TcpStream, ev: &mut LooperLoop<Top>, name: mio::Token)
    -> io::Result<Client> {
        let mut ircsock = IrcStream::new(sock);
        try!(ircsock.register(name, ev));
        Ok(Client {
            name: name,
            fmt: ctx.ircd.formatter(),
            sock: ircsock,
            state: ClientState::start()
        })
    }
}

impl ClientState {
    fn start() -> ClientState { ClientState::Pending(PendingData::new()) }
}

impl Pollable<Top> for Client {
    /// Called to indicate data is ready on the client's socket.
    fn ready(&mut self, ctx: &mut Top, act: &mut LooperActions<Top>) -> io::Result<()> {
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

            // take_mut::take() will *exit* on panic, so no panics!
            take_mut::take(state, |state| match state {
                ClientState::Pending(mut data) => {
                    data.handle_pending(ctx, &m, fmt, sock);
                    data.try_promote(ctx, fmt, sock)
                },

                ClientState::Active(mut data) => {
                    data.handle_active(ctx, &m, fmt, sock);
                    ClientState::Active(data)
                },
            });

            None
        }));

        Ok(())
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
        ctx: &mut Top,
        m: &Message,
        fmt: &IrcFormatter,
        sock: &IrcStream
    ) {
        match m.verb {
            b"PASS" => match to_string(m.args[0]) {
                Some(s) => self.password = Some(s),
                None => info!("password must be valid UTF-8!"),
            },

            b"NICK" => match to_string(m.args[0]) {
                Some(s) => self.nickname = Some(s),
                None => info!("nickname must be valid UTF-8!"),
            },

            b"USER" => {
                let user = to_string(m.args[0]);
                let real = to_string(m.args[3]);
                if user.is_some() && real.is_some() {
                    self.username = user;
                    self.realname = real;
                } else {
                    info!("username and realname must be valid UTF-8!");
                }
            },

            _ => { }
        }
    }

    fn try_promote(self, ctx: &mut Top, fmt: &IrcFormatter, sock: &IrcStream) -> ClientState {
        let promotion = match Promotion::from_pending(self) {
            Ok(promotion) => promotion,
            Err(data) => {
                // user is missing something
                return ClientState::Pending(data);
            }
        };

        let identity = ctx.edit(|w| w.create_temp_identity());

        fmt.numeric(sock, RPL_WELCOME, &[]);
        fmt.numeric(sock, RPL_ISUPPORT, &[b"CHANTYPES=#"]);

        ClientState::Active(ActiveData {
            identity: identity,
        })
    }
}

struct ActiveData {
    identity: Id<Identity>,
}

impl ActiveData {
    fn handle_active<'c>(
        &mut self,
        ctx: &mut Top,
        m: &Message,
        fmt: &IrcFormatter,
        sock: &IrcStream
    ) {
        match m.verb {
            b"JOIN" => ctx.edit(|world| {
                let chan = {
                    let chname = match to_string(m.args[0]) {
                        Some(name) => name,
                        None => { info!("channel name must be valid UTF-8!"); return; }
                    };

                    match world.channel_name_owner(&chname).cloned() {
                        Some(chan) => chan,
                        None => {
                            let chan = world.create_channel();
                            world.channel_claim(chan.clone(), chname.clone());
                            chan
                        }
                    }
                };

                world.channel_user_add(chan, self.identity.clone());
            }),

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
