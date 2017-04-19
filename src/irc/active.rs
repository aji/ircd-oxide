//! Active (fully-registered) client connection handling

use futures::Future;

use irc;
use irc::driver::Client;
use irc::send::Sender;

use world::World;

/// An active client
pub struct Active {
    world: World,
    _out: Sender,
    nick: String,
}

impl Active {
    /// Creates a new `Active`
    pub fn new(world: World, out: Sender, nick: String) -> Active {
        Active { world: world, _out: out, nick: nick }
    }

    pub fn handle(self, m: irc::Message) -> irc::Op<Client> {
        irc::Op::boxed(self.handle_easy(m).map(Client::Active))
    }

    fn handle_easy(mut self, m: irc::Message) -> irc::Op<Active> {
        debug!(" -> {:?}", m);

        match &m.verb[..] {
            b"JOIN" => {
                let chan = "#foo".to_string();
                let op = self.world.join_user(chan, self.nick.clone())
                    .and_then(move |_| Ok(self))
                    .map_err(|_| irc::Error::Other("part fail"));
                irc::Op::boxed(op)
            },

            b"PART" => {
                let chan = "#foo".to_string();
                let op = self.world.part_user(chan, self.nick.clone())
                    .and_then(move |_| Ok(self))
                    .map_err(|_| irc::Error::Other("part fail"));
                irc::Op::boxed(op)
            },

            b"PRIVMSG" => {
                let chan = "#foo".to_string();
                let message = "hello".to_string();
                let op = self.world.message(chan, self.nick.clone(), message)
                    .and_then(move |_| Ok(self))
                    .map_err(|_| irc::Error::Other("message fail"));
                irc::Op::boxed(op)
            },

            _ => {
                irc::Op::ok(self)
            }
        }
    }
}
