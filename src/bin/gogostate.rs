extern crate ircd;

use ircd::common::Sid;
use ircd::state::identity::Identity;
use ircd::state::id::IdGenerator;
use ircd::state::world::World;

use std::io;
use std::io::prelude::*;

struct Runner {
    sid: Sid,
    identities: IdGenerator<Identity>,
    world: World,
}

impl Runner {
    fn new(sid: Sid) -> Runner {
        Runner {
            sid: sid,
            identities: IdGenerator::new(sid),
            world: World::new()
        }
    }

    fn line(&mut self, line: String) {
        let fields: Vec<&str> = line
            .split(" ")
            .filter(|s| s.len() != 0)
            .collect();

        if fields.len() == 0 {
            return;
        }

        match fields[0] {
            "identity" => match fields.get(1) {
                Some(&"add") => {
                    let id = self.identities.next();
                    let identity = Identity::new(id.clone(), false);
                    println!("inserting {:?}", id);
                    self.world.identities_mut().insert(id, identity);
                },
                Some(c) => {
                    println!("identity: {}: unknown subcommand", c);
                },
                None => {
                    println!("identity: missing subcommand");
                },
            },

            "counter" => {
                println!("the counter reads {}", self.world.counter());
            },

            "inc" => {
                *self.world.counter_mut() += 1;
                println!("the counter now reads {}", self.world.counter());
            },

            c => println!("{}: unknown command", c)
        }
    }
}

fn prompt() {
    print!("ircd> ");
    io::stdout().flush().ok().expect("flush failed!");
}

fn main() {
    let mut runner = Runner::new(Sid::new("RUN"));
    let stdin = io::stdin();
    prompt();
    for line in stdin.lock().lines() {
        runner.line(line.unwrap());
        prompt();
    }
}
