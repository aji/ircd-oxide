use irc::active::Active;
use irc::driver::State;
use irc::driver::ClientOp;
use irc::message::Message;
use irc::pluto::Pluto;
use irc::send::SendHandle;

pub struct Pending {
    pluto: Pluto,
    out: SendHandle,
    counter: usize
}

impl Pending {
    pub fn new(pluto: Pluto, out: SendHandle) -> Pending {
        Pending { pluto: pluto, out: out, counter: 0 }
    }
}

impl State for Pending {
    type Next = Active;

    fn handle(mut self, m: Message) -> ClientOp<Self> {
        info!(" -> {:?}", m);

        match &m.verb[..] {
            b"REGISTER" => {
                self.out.send(&b"registering you...\r\n"[..]);
                self.counter += 1;
            },

            b"SPECIAL" => {
                self.out.send(&b"you are not special yet\r\n"[..]);
            },

            _ => { }
        }

        ClientOp::ok(self)
    }

    fn transition(self) -> Result<Active, Pending> {
        if self.counter > 2 {
            Ok(Active::new(self.pluto, self.out))
        } else {
            Err(self)
        }
    }
}
