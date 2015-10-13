// state/mod.rs -- state handling logic
// Copyright (C) 2015 Alex Iadicicco <http://ajitek.net>

//! State handling

pub mod channel;
pub mod clock;
pub mod masklist;

pub use self::channel::Channel;
pub use self::channel::Topic;
pub use self::clock::Clock;
pub use self::masklist::MaskList;

/// `StateItem` will be implemented by all updatable pieces of global shared
/// state. The `merge()` operation will be used to perform all updates, and
/// must have the following properties:
///
///   * Existence of an identity: `merge(X, e)` = `X` for some identity `e`
///   * Idempotency: `merge(X, X)` = `X`
///   * Commutativity: `merge(A, B)` = `merge(B, A)`
///   * Associativity: `merge(merge(A, B), C)` = `merge(A, merge(B, C))`
///
/// Examples of familiar operations that are `merge`-like are the set union
/// operator and numeric maximum function.
///
/// Given these invariant properties, and notating `merge(A, B)` as a binary
/// operator *A* &curren; *B*, we can drop grouping and disregard or add
/// grouping and duplicates as needed. Consider the following equivalences:
///
///   * ( *A* &curren; *B* &curren; *C* ) &curren; *D*
///     = *A* &curren; *B* &curren; *C* &curren; *D*
///   * ( *A* &curren; *B* ) &curren; ( *D* &curren; *C* )
///     = *A* &curren; *B* &curren; *C* &curren; *D*
///   * ( *A* &curren; *B* &curren; *C* ) &curren; *C*
///     = *A* &curren; *B* &curren; *C*
///   * etc.
///
/// It's clear then that, no matter what order new state is being merged in, as
/// long as all nodes receive all updated pieces of state, they will eventually
/// agree on what the most accurate state of that data is.
///
/// This works excellently for IRC, as IRC deals in many small pieces of state
/// with simple merging rules based on things like real-world time.
pub trait StateItem {
    /// Constructs the identity state
    fn identity() -> Self;

    /// This is the most important operation that any piece of state should
    /// implement. See the trait-level documentation for what requirements this
    /// function should have.
    fn merge(&mut self, other: &Self) -> &mut Self;
}
