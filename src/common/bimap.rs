// common/bimap.rs -- map from key pairs
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net/>
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

//! A bimap m is a partial function m : A x B &rarr; Option<T>, mapping pairs of A and B
//! to instances of T, so m(a, b) = t is true when the bimap contains (a, b) -> t. Note that,
//! though partial, m is a function, so m(a, b) can only have one t, if m is defined for that
//! pair.
//!
//! These docs will use set theory notation

use std;
use std::collections::HashMap;
use std::hash::Hash;

type Ti = u32;
type Ai = u32;
type Bi = u32;

static nothingA: [Ai; 0] = [];
static nothingB: [Bi; 0] = [];

struct Bimap<A: Eq + Hash, B: Eq + Hash, T> {
    aa: HashMap<A, Ai>,
    bb: HashMap<B, Bi>,
    tt: HashMap<Ti, T>,

    apair: HashMap<Ai, Vec<Bi>>,
    bpair: HashMap<Bi, Vec<Ai>>,
    pairs: HashMap<(Ai, Bi), Ti>,

    na: Ai,
    nb: Bi,
    nt: Ti,
}

impl<A: Eq + Hash, B: Eq + Hash, T> Bimap<A, B, T> {
    fn new() -> Bimap<A, B, T> {
        Bimap {
            tt: HashMap::new(),
            aa: HashMap::new(),
            bb: HashMap::new(),

            apair: HashMap::new(),
            bpair: HashMap::new(),
            pairs: HashMap::new(),

            na: 0,
            nb: 0,
            nt: 0,
        }
    }

    // Inserts m(a, b) = t
    fn insert(&mut self, a: A, b: B, t: T) {
        let na = &mut self.na;
        let nb = &mut self.nb;

        let ai = self.aa.entry(a).or_insert_with(||{ *na += 1; *na });
        let bi = self.bb.entry(b).or_insert_with(||{ *nb += 1; *nb });

        let ti = { self.nt += 1; self.nt };
        self.tt.insert(ti, t);

        let apair = self.apair.entry(*ai).or_insert_with(|| Vec::new());
        let bpair = self.bpair.entry(*bi).or_insert_with(|| Vec::new());

        apair.push(*bi);
        bpair.push(*ai);

        self.pairs.insert((*ai, *bi), ti);
    }

    // m(a, b)
    fn get(&self, a: &A, b: &B) -> Option<&T> {
        let ai = self.aa.get(a).cloned().unwrap_or(0);
        let bi = self.bb.get(b).cloned().unwrap_or(0);

        if ai == 0 || bi == 0 {
            None
        } else {
            self.tt.get(self.pairs.get(&(ai, bi)).unwrap_or(&0))
        }
    }

    // all t where \exists a such that m(a, b) = t
    fn all_a(&self, b: &B) -> AllA<T> {
        let bi = self.bb.get(b).cloned().unwrap_or(0);
        let iter = self.bpair.get(&bi).map(|v| v.iter()).unwrap_or(nothingA.iter());
        AllA::new(&self.pairs, &self.tt, bi, iter)
    }

    // all t where \exists b such that m(a, b) = t
    fn all_b(&self, a: &A) -> AllB<T> {
        let ai = self.aa.get(a).cloned().unwrap_or(0);
        let iter = self.apair.get(&ai).map(|v| v.iter()).unwrap_or(nothingB.iter());
        AllB::new(&self.pairs, &self.tt, ai, iter)
    }

    // any t where \exists a such that m(a, b) = t
    fn any_a(&self, b: &B) -> Option<&T> {
        let pairs = &self.pairs;
        let tt = &self.tt;

        let bi = self.bb.get(b).cloned().unwrap_or(0);

        self.bpair.get(&bi)
            .and_then(|aa| aa.first())
            .and_then(|ai| pairs.get(&(*ai, bi)))
            .and_then(|ti| tt.get(ti))
    }

    // any t where \exists b such that m(a, b) = t
    fn any_b(&self, a: &A) -> Option<&T> {
        let pairs = &self.pairs;
        let tt = &self.tt;

        let ai = self.aa.get(a).cloned().unwrap_or(0);

        self.apair.get(&ai)
            .and_then(|bb| bb.first())
            .and_then(|bi| pairs.get(&(ai, *bi)))
            .and_then(|ti| tt.get(ti))
    }
}

pub struct AllA<'m, T: 'static> {
    pairs: &'m HashMap<(Ai, Bi), Ti>,
    tt: &'m HashMap<Ti, T>,
    bi: Bi,
    iter: std::slice::Iter<'m, Ai>,
}

impl<'m, T> AllA<'m, T> {
    fn new(
        pairs: &'m HashMap<(Ai, Bi), Ti>,
        tt: &'m HashMap<Ti, T>,
        bi: Bi,
        iter: std::slice::Iter<'m, Ai>,
    ) -> AllA<'m, T> {
        AllA { pairs: pairs, tt: tt, bi: bi, iter: iter }
    }
}

impl<'m, T> Iterator for AllA<'m, T> {
    type Item = &'m T;

    fn next(&mut self) -> Option<Self::Item> {
        let pairs = self.pairs;
        let tt = self.tt;
        let bi = self.bi;
        self.iter.next()
            .and_then(|ai| pairs.get(&(*ai, bi)))
            .and_then(|ti| tt.get(ti))
    }
}

pub struct AllB<'m, T: 'static> {
    pairs: &'m HashMap<(Ai, Bi), Ti>,
    tt: &'m HashMap<Ti, T>,
    ai: Ai,
    iter: std::slice::Iter<'m, Bi>,
}

impl<'m, T> AllB<'m, T> {
    fn new(
        pairs: &'m HashMap<(Ai, Bi), Ti>,
        tt: &'m HashMap<Ti, T>,
        ai: Ai,
        iter: std::slice::Iter<'m, Bi>,
    ) -> AllB<'m, T> {
        AllB { pairs: pairs, tt: tt, ai: ai, iter: iter }
    }
}

impl<'m, T> Iterator for AllB<'m, T> {
    type Item = &'m T;

    fn next(&mut self) -> Option<Self::Item> {
        let pairs = self.pairs;
        let tt = self.tt;
        let ai = self.ai;
        self.iter.next()
            .and_then(|bi| pairs.get(&(ai, *bi)))
            .and_then(|ti| tt.get(ti))
    }
}
