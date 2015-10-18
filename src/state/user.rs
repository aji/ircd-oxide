// state/user.rs -- user state management logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! User state management logic

use irc::IrcString;
use state::clock;
use state::diff;
use state::id::Id;
use state::StateItem;

/// A user's identity
pub struct Identity;

/// A nickname
pub struct Nickname {
    id: Id<Nickname>,
    text: IrcString,
    claim: NicknameClaim
}

impl Nickname {
    /// Get the globally unique ID for this nickname.
    pub fn id(&self) -> &Id<Nickname> { &self.id }

    /// Get the text of this nickname.
    pub fn text(&self) -> &IrcString { &self.text }

    /// Get the claim to this nickname.
    pub fn claim(&self) -> &NicknameClaim { &self.claim }
}

/// A claim to a nickname.
///
/// To explain how the merge rules for claims work, consider a history of all
/// information about a claim, i.e. the set of all new claims and claim
/// expirations. We want to pick the *oldest* claim after the *newest*
/// expiration. This matches the intuition that nicknames are claimed on a first
/// come first serve basis.
///
/// We want to be able to capture this behavior with the smallest amount of
/// data possible, preferably so that it can't grow without bound.
///
/// What we do then is store the clock of the current valid claim, and the clock
/// of the expiration that the claim supersedes. When we do this, the "current
/// valid claim" is *also* the first claim after the stored expiration. This
/// data forms the "claim object".
///
/// When a new claim object arrives, there are a small handful of scenarios to
/// consider:
///
///   * The expiration clocks match. This means that both claims were derived
///     from the same expiration, and we simply prefer the older claim.
///
///   * The expiration clocks do not match. This means that we have two claims
///     and two expirations to reconcile. We then simply pick the newer of the
///     two expirations, and the older claim to occur after that expiration.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NicknameClaim {
    expired: clock::Clock,
    claimed: clock::Clock,
    owner: Option<Id<Identity>>,
}

impl diff::AtomDiffable for NicknameClaim { }

impl StateItem for NicknameClaim {
    fn identity() -> NicknameClaim {
        NicknameClaim {
            expired: clock::Clock::identity(),
            claimed: clock::Clock::identity(),
            owner: None,
        }
    }

    fn merge(&mut self, other: &NicknameClaim) -> &mut NicknameClaim {
        if self.expired == other.expired {
            // expirations are equal, just take the older claim
            if self.claimed > other.claimed {
                *self = other.clone();
            }
        } else {
            // reconcile the expiration first
            if self.expired < other.expired {
                // their expiration is newer
                self.expired = other.expired;
            }

            // with one expiration, reconcile the claim: if we have expired, or
            // they have not expired and have an older claim, then pick them.
            if self.expired > self.claimed ||
                    (self.claimed > other.claimed &&
                    other.claimed > self.expired) {
                self.claimed = other.claimed;
                self.owner = other.owner.clone();
            }
        }

        self
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
fn assert_claim_merge(
    exX: clock::Clock, clX: clock::Clock, ownX: Option<&Id<Identity>>,
    exS: clock::Clock, clS: clock::Clock, ownS: Option<&Id<Identity>>,
    exO: clock::Clock, clO: clock::Clock, ownO: Option<&Id<Identity>>,
) {

    let      x = NicknameClaim { expired: exX, claimed: clX, owner: ownX.cloned() };
    let mut  s = NicknameClaim { expired: exS, claimed: clS, owner: ownS.cloned() };
    let      o = NicknameClaim { expired: exO, claimed: clO, owner: ownO.cloned() };

    s.merge(&o);

    if x != s {
        println!("failed:");
        println!("expected: {:?} {:?} {:?}", x.expired, x.claimed, x.owner.unwrap());
        println!("     got: {:?} {:?} {:?}", s.expired, s.claimed, s.owner.unwrap());
        panic!();
    } else {
        println!("passed");
    }
}

#[test]
fn test_nickname_claim_merge() {
    use state::id::IdGenerator;
    use state::clock::Clock;

    let mut idgen: IdGenerator<Identity> = IdGenerator::new(0);

    let t0 = Clock::at(0);
    let t1 = Clock::at(1);
    let t2 = Clock::at(2);
    let t3 = Clock::at(3);
    let is = idgen.next();
    let io = idgen.next();

    // matching expirations

    assert_claim_merge(t0, t1, Some(&is),
                       t0, t1, Some(&is), t0, t2, Some(&io));
    assert_claim_merge(t0, t1, Some(&io),
                       t0, t2, Some(&is), t0, t1, Some(&io));

    // mismatched expirations

    // t0  t1  t2  t3
    // exS exO clS clO => exO clS
    assert_claim_merge(t1, t2, Some(&is),
                       t0, t2, Some(&is), t1, t3, Some(&io));
    // exS exO clO clS => exO clO
    assert_claim_merge(t1, t2, Some(&io),
                       t0, t3, Some(&is), t1, t2, Some(&io));
    // exS clS exO clO => exO clO
    assert_claim_merge(t2, t3, Some(&io),
                       t0, t1, Some(&is), t2, t3, Some(&io));
    // exO exS clS clO => exS clS
    assert_claim_merge(t1, t2, Some(&is),
                       t1, t2, Some(&is), t0, t3, Some(&io));
    // exO exS clO clS => exS clO
    assert_claim_merge(t1, t2, Some(&io),
                       t1, t3, Some(&is), t0, t2, Some(&io));
    // exO clO exS clS => exS clS
    assert_claim_merge(t2, t3, Some(&is),
                       t2, t3, Some(&is), t0, t1, Some(&io));
}
