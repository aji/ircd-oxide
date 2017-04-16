use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use futures::Future;
use futures::Stream;

use tokio_core::reactor::Handle;

use irc::send::Sender;
use world::World;
use world::WorldEvent;

struct PoolInner {
    users: HashMap<String, Sender>,
    chans: HashMap<String, HashSet<String>>,
}

impl PoolInner {
    fn new() -> PoolInner {
        PoolInner {
            users: HashMap::new(),
            chans: HashMap::new(),
        }
    }

    fn dispatch(&mut self, event: &WorldEvent) {
        info!("event: {:?}", event);

        match *event {
            WorldEvent::UserJoin(ref chan, ref user) => {
                self.chans
                    .entry(chan.clone())
                    .or_insert_with(|| HashSet::new())
                    .insert(user.clone());
                self.send_to_chan(chan, None,
                    format!(":{} JOIN {}", user, chan));
            },

            WorldEvent::UserPart(ref chan, ref user) => {
                self.send_to_chan(chan, None,
                    format!(":{} PART {}", user, chan));
                self.chans.get_mut(chan).map(|c| c.remove(user));
            },

            WorldEvent::Message(ref chan, ref user, ref message) => {
                self.send_to_chan(chan, Some(user),
                    format!(":{} PRIVMSG {} :{}", user, chan, message));
            },
        }
    }

    fn send_to_chan(&mut self, chan: &String, omit: Option<&String>, line: String) {
        let users = match self.chans.get(chan) {
            Some(users) => users,
            None => return,
        };

        for user in users.iter() {
            if Some(user) == omit {
                continue;
            }

            if let Some(mut out) = self.users.get_mut(user) {
                out.send(line.as_bytes());
                out.send(b"\r\n");
            }
        }
    }
}

#[derive(Clone)]
pub struct Pool {
    inner: Rc<RefCell<PoolInner>>
}

impl Pool {
    pub fn new() -> Pool {
        Pool { inner: Rc::new(RefCell::new(PoolInner::new())) }
    }

    pub fn bind(&self, handle: &Handle, world: &mut World) {
        let inner = self.inner.clone();

        handle.spawn(world.events().for_each(move |event| {
            inner.borrow_mut().dispatch(&*event);
            Ok(())
        }));
    }

    pub fn add_user(&mut self, name: String, out: Sender) {
        self.inner.borrow_mut().users.insert(name, out);
    }
}
