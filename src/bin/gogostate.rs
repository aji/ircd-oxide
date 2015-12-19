extern crate ircd;

use ircd::state::world::World;

use std::io;
use std::io::prelude::*;

struct Runner {
    world: World,
}

impl Runner {
    fn new() -> Runner {
        Runner {
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
    let mut runner = Runner::new();
    let stdin = io::stdin();
    prompt();
    for line in stdin.lock().lines() {
        runner.line(line.unwrap());
        prompt();
    }
}
