use futures::Future;

use irc::ClientError;
use irc::message::Message;
use irc::pluto::Pluto;
use irc::pluto::PlutoReader;
use irc::pluto::PlutoWriter;
use irc::driver::State;
use irc::driver::ClientOp;
use irc::send::SendHandle;
use irc::pending::Pending;

pub struct Active {
    pluto: Pluto,
    out: SendHandle,
    wants_close: bool
}

impl Active {
    pub fn new(pluto: Pluto, out: SendHandle) -> Active {
        Active { pluto: pluto, out: out, wants_close: false }
    }
}

impl State for Active {
    type Next = ();

    fn handle(mut self, m: Message) -> ClientOp<Self> {
        info!(" -> {:?}", m);

        match &m.verb[..] {
            b"REGISTER" => {
                self.out.send(&b"you're already registered\r\n"[..]);
            },

            b"SPECIAL" => {
                self.out.send(&b"very special!\r\n"[..]);
                // TODO: figure out how to get rid of the clone on this next line:
                let op = self.pluto.clone().tx(move |p| {
                    let next = p.get() + 1;
                    self.out.send(format!("incrementing to {}\r\n", next).as_bytes());
                    p.set(next);
                    self
                }).map(|mut client| {
                    client.out.send(&b"all done!\r\n"[..]);
                    client
                }).map_err(|_| ClientError::Other("ouch"));
                return ClientOp::boxed(op);
            },

            b"CLOSE" => {
                self.wants_close = true;
            },

            _ => { }
        }

        ClientOp::ok(self)
    }

    fn handle_eof(mut self) -> ClientOp<Self> {
        self.wants_close = true;
        ClientOp::ok(self)
    }

    fn transition(mut self) -> Result<(), Active> {
        if self.wants_close {
            self.out.send(&b"closing you...\r\n"[..]);
            Ok(())
        } else {
            Err(self)
        }
    }
}
