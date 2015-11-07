// macros.rs -- Various macros used throughout
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Macros

macro_rules! irc {
    ($writer:expr,) => {
        ::std::io::Write::write($writer, b"\r\n")
    };

    ($writer:expr, $arg:expr,) => {
        ::std::io::Write::write(
            $writer,
            ::std::convert::AsRef::<[u8]>::as_ref(&$arg)
        )
    };

    ($writer:expr, $arg:expr) => {
        match irc!($writer, $arg,) {
            Err(e) => Err(e),
            Ok(_) => irc!($writer,)
        }
    };

    ($writer:expr, $arg:expr, $($args:tt)*) => {
        match irc!($writer, $arg,) {
            Err(e) => Err(e),
            Ok(_) => match ::std::io::Write::write($writer, b" ") {
                Err(e) => Err(e),
                Ok(_) => irc!($writer, $($args)*),
            }
        }
    }
}
