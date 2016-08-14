// irc/numeric.rs -- numerics macros
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

macro_rules! rpl_welcome {
    ($f:expr, $s:expr) => ($f.numeric($s, 1, format_args!(":Welcome!")))
}

macro_rules! rpl_isupport {
    ($f:expr, $s:expr, $args:tt) => ($f.numeric($s, 5, format_args!("{} :are supported", $args)))
}
