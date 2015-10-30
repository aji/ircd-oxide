// state/claim.rs -- claim handling
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! Claim handling
//!
//! Claims are the optimistic locks that make `ircd-oxide` work. They are used
//! in places where data presented to users appears to exist in a shared global
//! namespace, such as channel names and nicknames. For such data, `ircd-oxide`
//! attempts to make a "claim" on that data. Data can only be claimed for things
//! that are guaranteed to be unique (the "owner"). Examples of things that are
//! guaranteed to be unique are channel IDs and user identities.
//!
//! Claims themselves are simple: you either have a claim over some data, or you
//! don't. The difficult part is that your claim to that data may be superseded
//! at any moment. This module only deals with detecting such events, and
//! ensuring that conflicts are reconciled in the same way across all nodes.
//! Handling the loss of a claim is the responsibility of the user the claim is
//! issued to.
//!
//! Take nicknames as a simple example. When a user wants to use a nickname for
//! which the server they are connected to sees no claim, the server creates a
//! claim and broadcasts it. If at any point the claim is superseded, the server
//! must indicate to the client that they are no longer using that nickname,
//! usually with something like a forced nickname change to a unique ID.
//!
//! Expirations are a slightly more subtle feature, and act like "tombstones".
//! When a user releases their claim to some data, we can't just forget the
//! claim existed. Since older claims always win, we can only create a newer
//! claim using a prior expiration as justification. If we broadcast a newer
//! claim without justifying it, we are relying on all receiving servers having
//! no older claim. Since we cannot reliably verify at any time that all servers
//! have seen an expiration, we keep track of the expiration for all data.
//!
//! In the future, to prevent the unbounded growth of expired claims, we may use
//! some kind of strong consistency to clean up old expirations.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::hash::Hash;

use state::diff;
use state::Clock;
use state::Id;
use state::StateItem;

/// A claim object.
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
///
/// ***Claim.*** The following two models of "claims" as state items will
/// produce the same active claim after a merge:
///
///   1. The "claim object" is a set of all timestamped claim attempts and
///      timestamped expirations. The active claim is the oldest claim that
///      comes after the newest expiration. If no such claim exists, there is no
///      active claim. Merging claim objects is simply set union.
///
///   2. The "claim object" is a single timestamped claim attempt and a single
///      timestamped expiration. The claim is active if the expiration is older
///      than the claim attempt. Merging claim objects is performed using the
///      method described above.
///
/// ***Proof.*** Consider two claim objects to be merged under the first model,
/// call them *A* and *B*. Each claim object has a newer expiration, *Ea* and
/// *Eb* respectively. Using the method described, we can pick an active claim
/// for each set, call them *Ca* and *Cb* respectively. When we merge the sets,
/// one of *Ea* or *Eb* will be the newer expiration. WLOG, let *Ea* be the
/// newer expiration. At this point, the active claim in the new set will be
/// *only* either *Ca* or *Cb*. To make a contradiction, suppose in the newly
/// merged set that some claim *Cx*, distinct from *Ca* or *Cb*, is the active
/// claim. In other words, T(*Cx*) < T(*Ca*) and T(*Cx*) < T(*Cb*), where T(*e*)
/// indicates the (unique) time of event *e*. Note also that T(*Eb*) < T(*Ea*) <
/// T(*Cx*). If *Cx* were in starting set *A*, then we would have T(*Ea*) <
/// T(*Cx*) < T(*Ca*), and *Ca* would not be the valid claim from that as
/// defined above. Similarly, if *Cx* were in starting set *B*, then *Cb* would
/// not be the valid claim from that set. Since *Cx* is in neither *A* nor *B*,
/// it is not in the union of *A* and *B*, and so cannot exist as we have
/// defined it. Therefore, the newly valid claim after the union must be either
/// *Ca* or *Cb*.
///
/// I will finish this proof later, but you hopefully see where I'm going with
/// this. We only need to store and transmit *Ea*, *Eb*, *Ca*, and *Cb*, to
/// determine the new expiration and valid claim.
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

    /// Determines if the claim is valid
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

/// A map of claims
pub struct ClaimMap<Owner: 'static, Over: 'static + Eq + Hash> {
    map: HashMap<Over, Claim<Owner, Over>>,
}

impl<Owner: 'static, Over: 'static + Eq + Hash> ClaimMap<Owner, Over> {
    pub fn new() -> ClaimMap<Owner, Over> {
        ClaimMap {
            map: HashMap::new()
        }
    }

    pub fn is_claimed(&self, k: &Over) -> bool {
        self.map.get(k).map(|cl| cl.is_valid()).unwrap_or(false)
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
    use util::Sid;

    let mut idgen: IdGenerator<()> = IdGenerator::new(Sid::identity());

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
