// state/claim.rs -- claim handling
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>

//! Claim handling
//!
//! Claims are the optimistic locks that make `ircd-oxide` work.

use std::marker::PhantomData;

use state::diff;
use state::Clock;
use state::Id;
use state::StateItem;

/// A claim object
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
#[derive(Debug)]
pub struct Claim<Owner: 'static, Over: 'static> {
    expired: Clock,
    claimed: Clock,
    owner: Option<Id<Owner>>,
    _over: PhantomData<&'static mut Over>
}

impl<Owner: 'static, Over: 'static> Claim<Owner, Over> {
    /// Creates an empty claim object that can be superseded by any other claim.
    pub fn empty() -> Claim<Owner, Over> {
        Claim {
            expired: Clock::neg_infty(),
            claimed: Clock::pos_infty(),
            owner: None,
            _over: PhantomData
        }
    }

    /// Determines if the claim on the nickname is valid
    pub fn is_valid(&self) -> bool {
        self.claimed > self.expired
    }
}

impl<Owner: 'static, Over: 'static> PartialEq for Claim<Owner, Over> {
    fn eq(&self, other: &Claim<Owner, Over>) -> bool {
        self.expired  ==  other.expired &&
        self.claimed  ==  other.claimed &&
        self.owner    ==  other.owner
    }
}

impl<Owner: 'static, Over: 'static> Eq for Claim<Owner, Over> { }

impl<Owner: 'static, Over: 'static> Clone for Claim<Owner, Over> {
    fn clone(&self) -> Claim<Owner, Over> {
        Claim {
            expired: self.expired,
            claimed: self.claimed,
            owner: self.owner.clone(),
            _over: PhantomData,
        }
    }
}

impl<Owner: 'static, Over: 'static> diff::AtomDiffable for Claim<Owner, Over> { }

impl<Owner: 'static, Over: 'static> StateItem for Claim<Owner, Over> {
    fn merge(&mut self, other: &Claim<Owner, Over>) -> &mut Claim<Owner, Over> {
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
fn assert_claim_merge<Owner: 'static>(
    exX: Clock, clX: Clock, ownX: Option<&Id<Owner>>,
    exS: Clock, clS: Clock, ownS: Option<&Id<Owner>>,
    exO: Clock, clO: Clock, ownO: Option<&Id<Owner>>,
) {

    let x: Claim<Owner, ()> = Claim {
        expired: exX, claimed: clX, owner: ownX.cloned(), _over: PhantomData };
    let mut s = Claim {
        expired: exS, claimed: clS, owner: ownS.cloned(), _over: PhantomData };
    let o = Claim {
        expired: exO, claimed: clO, owner: ownO.cloned(), _over: PhantomData };

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
fn test_claim_merge() {
    use state::IdGenerator;

    let mut idgen: IdGenerator<()> = IdGenerator::new(0);

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
