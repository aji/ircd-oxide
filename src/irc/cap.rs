// irc/cap.rs -- client capabilities
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Client capabilities

// THINGS TO UPDATE WHEN ADDING A NEW CAPABILITY: (this is a small file, but
// adding caps is a non-automatic process that tests can't really catch):
//
//   - The constants in `Caps`
//   - The functions on `ClientCaps`
//   - The `FromStr` impl (which `of` uses)
//   - `worthless_test`

use std::str::FromStr;

#[allow(dead_code)]
mod cap {
    bitflags! {
        pub flags Caps: u16 {
            const MULTI_PREFIX       = 0b_00000000_00000001,
            const ACCOUNT_NOTIFY     = 0b_00000000_00000010,
            const AWAY_NOTIFY        = 0b_00000000_00000100,
            const EXTENDED_JOIN      = 0b_00000000_00001000,
        }
    }
}

/// An immutable client capability set.
pub struct ClientCaps {
    caps: cap::Caps
}

impl ClientCaps {
    /// Creates an empty client capability set.
    pub fn empty() -> ClientCaps {
        ClientCaps { caps: cap::Caps::empty() }
    }

    /// Attemps to convert the given string into a `ClientCaps` representing a
    /// single capability. The string should be the IRCv3 name of the capability,
    /// such as `"multi-prefix"`.
    pub fn of(s: &str) -> Option<ClientCaps> {
        match FromStr::from_str(s) {
            Ok(c) => Some(c),
            Err(_) => None
        }
    }

    /// Modifies `self` in-place to contain the union of capabilities in `self`
    /// and `other`.
    pub fn add(&mut self, other: &ClientCaps) {
        self.caps = self.caps | other.caps;
    }

    /// Creates a new client capability set that includes both the capabilities
    /// in this set and `other`.
    pub fn with(&self, other: &ClientCaps) -> ClientCaps {
        ClientCaps { caps: self.caps | other.caps }
    }

    /// Indicates whether the `multi-prefix` capability is enabled.
    pub fn multi_prefix(&self) -> bool {
        self.caps.contains(cap::MULTI_PREFIX)
    }

    /// Indicates whether the `account-notify` capability is enabled.
    pub fn account_notify(&self) -> bool {
        self.caps.contains(cap::ACCOUNT_NOTIFY)
    }

    /// Indicates whether the `away-notify` capability is enabled.
    pub fn away_notify(&self) -> bool {
        self.caps.contains(cap::AWAY_NOTIFY)
    }

    /// Indicates whether the `extended-join` capability is enabled.
    pub fn extended_join(&self) -> bool {
        self.caps.contains(cap::EXTENDED_JOIN)
    }
}

impl FromStr for ClientCaps {
    type Err = ();

    fn from_str(s: &str) -> Result<ClientCaps, ()> {
        Ok(match s {
            "multi-prefix"        => ClientCaps { caps: cap::MULTI_PREFIX },
            "account-notify"      => ClientCaps { caps: cap::ACCOUNT_NOTIFY },
            "away-notify"         => ClientCaps { caps: cap::AWAY_NOTIFY },
            "extended-join"       => ClientCaps { caps: cap::EXTENDED_JOIN },
            _ => return Err(())
        })
    }
}

#[test]
fn worthless_test() {
    // worthless because if this test breaks, then something is actually really
    // badly broken. additionally, new additions aren't covered by this test
    // unless the test is also updated. pretty worthless!

    assert!(ClientCaps::of("multi-prefix").unwrap().multi_prefix());
    assert!(ClientCaps::of("account-notify").unwrap().account_notify());
    assert!(ClientCaps::of("away-notify").unwrap().away_notify());
    assert!(ClientCaps::of("extended-join").unwrap().extended_join());

    // ping me if they ever standardize a capability called "poo" because I want
    // to implement it.
    assert!(ClientCaps::of("poo").is_none());
}
