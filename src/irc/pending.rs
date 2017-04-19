//! Code to listen for and drive pre-registration connections

use futures::Future;

use irc;
use irc::active::Active;
use irc::driver::Client;
use irc::send::Sender;

use world::World;

pub struct Pending {
    world: World,
    out: Sender,
    nick: Option<String>
}

impl Pending {
    pub fn new(world: World, out: Sender) -> Pending {
        Pending {
            world: world,
            out: out,
            nick: None,
        }
    }

    pub fn handle(mut self, m: irc::Message) -> irc::Op<Client> {
        debug!(" -> {:?}", m);

        if b"NICK" == &m.verb[..] && m.args.len() > 0 {
            if let Ok(nick) = String::from_utf8(m.args[0].to_vec()) {
                self.nick = Some(nick);
            }
        }

        if let Some(nick) = self.nick.as_ref().cloned() {
            self.out.send(&b"auth successful\r\n"[..]);

            let op = self.world.add_user(nick.clone()).and_then(move |_| {
                self.out.send(&b"welcome!\r\n"[..]);
                let active = Active::new(self.world, self.out, nick);
                Ok(Client::Active(active))
            }).map_err(|_| irc::Error::Other("register error"));

            irc::Op::boxed(op)

        } else {
            irc::Op::ok(Client::Pending(self))
        }
    }
}
