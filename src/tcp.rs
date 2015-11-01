// tcp.rs -- TCP handling utilities
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! TCP helpers

use std::cell::RefCell;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::rc::Rc;

/// Write end of a TCP stream
pub struct TcpWriter {
    sock: Rc<RefCell<TcpStream>>
}

impl Write for TcpWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sock.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sock.borrow_mut().flush()
    }
}

/// Read end of a TCP stream
pub struct TcpReader {
    sock: Rc<RefCell<TcpStream>>
}

impl Read for TcpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.sock.borrow_mut().read(buf)
    }
}

/// Splits a TCP stream into its read and write ends
pub fn split(s: TcpStream) -> (TcpWriter, TcpReader) {
    let rc = Rc::new(RefCell::new(s));

    (
        TcpWriter { sock: rc.clone() },
        TcpReader { sock: rc }
    )
}
