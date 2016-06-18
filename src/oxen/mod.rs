// oxen/mod.rs -- oxen server-to-server protocol
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Oxen is the cluster membership and messaging protocol.
//!
//! Oxen provides reliable in-order delivered once messaging, and reliable
//! out-of-order delivered at-least-once messaging. The in-order delivered once
//! case can be seen as extra handling on top of the out-of-order delivered
//! at-least-once case. For such messages, a message numbering scheme is used to
//! detect duplicates and correctly order messages.
//!
//! Because of ircd-oxide's state checkpointing and merging capabilities, the
//! out-of-order delivered at-least-once case is good enough for synchronizing
//! state. For cases that need to map more closely to traditional IRC, such as
//! PRIVMSG, the in-order delivered once functionality can be used.
//!
//! See the section of the manual on Oxen for more details on how the protocol
//! works and what it guarantees.

pub mod core;
pub mod data;
pub mod lc;

pub use self::core::Oxen;
pub use self::core::OxenEvent;
pub use self::core::OxenHandler;
pub use self::core::Timer;
