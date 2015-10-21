// oxen/back.rs -- backend API that Oxen is built on top of
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! The backend API that the Oxen protocol runs on top of.

use std::convert::From;
use std::hash::Hash;
use time::Timespec;
use time::Duration;

use util::Sid;
use xenc;

pub trait OxenBack {
    type Timer: Clone + Eq + Hash;

    fn get_time(&self) -> Timespec;

    fn queue_send(&mut self, peer: Sid, data: Vec<u8>);

    fn timer_set(&mut self, at: Duration) -> Self::Timer;

    fn timer_cancel(&mut self, timer: Self::Timer);

    fn queue_send_xenc<T>(&mut self, peer: Sid, data: T)
    where xenc::Value: From<T> {
        let mut vec = Vec::new();
        xenc::Value::from(data).write(&mut vec);
        self.queue_send(peer, vec);
    }
}
