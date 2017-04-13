//! A toy world, for testing
//!
//! The Pluto subsystem is designed to have as many of the same feature dimensions as a real
//! IRC state model, while ignoring all the complexity that would be introduced by actually
//! implementing such a thing. Pluto is designed to have the following properties in common
//! with an actual world:
//!
//!  * Shared data, which can be mutated atomically.
//!  * Update broadcasts (implemented with `common::observe`).

use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use futures::Poll;
use futures::Async;
use futures::Future;

use common::observe::Observable;
use common::observe::Observer;
use common::observe::Completion;

struct PlutoCore {
    val: u32,
    observable: Observable<u32>,
}

type PlutoRef = Rc<RefCell<PlutoCore>>;

impl PlutoCore {
    fn new() -> PlutoCore {
        PlutoCore { val: 0, observable: Observable::new() }
    }

    fn into_ref(self) -> PlutoRef {
        Rc::new(RefCell::new(self))
    }

    fn observer(&mut self) -> Observer<u32> {
        self.observable.observer()
    }

    fn send_update(&mut self) -> Completion {
        self.observable.put(self.val)
    }
}

/// A trait to read shared information using the Pluto data model
pub trait PlutoReader {
    fn get(&self) -> u32;
}

/// A trait to write shared information using the Pluto data model
pub trait PlutoWriter {
    fn set(&mut self, x: u32) -> ();
}

/// A readable and writable reference to shared information, only made available in the context
/// of a transaction.
pub struct PlutoTxContext {
    p: PlutoRef,
    val_changed: bool
}

impl PlutoTxContext {
    fn open(p: PlutoRef) -> PlutoTxContext {
        PlutoTxContext { p: p, val_changed: false }
    }

    fn finalize(self) -> Option<Completion> {
        if self.val_changed {
            Some(self.p.borrow_mut().send_update())
        } else {
            None
        }
    }
}

impl PlutoReader for PlutoTxContext {
    fn get(&self) -> u32 {
        self.p.borrow().val
    }
}

impl PlutoWriter for PlutoTxContext {
    fn set(&mut self, x: u32) {
        self.p.borrow_mut().val = x;
        self.val_changed = true;
    }
}

/// A cloneable reference to shared information.
#[derive(Clone)]
pub struct Pluto {
    p: PlutoRef
}

impl Pluto {
    /// Creates a new Pluto state world
    pub fn new() -> Pluto {
        Pluto { p: PlutoCore::new().into_ref() }
    }

    /// Creates a new transaction with the given body. See [`PlutoTx`](struct.PlutoTx.html)
    /// for more details.
    pub fn tx<F, T>(&self, body: F) -> PlutoTx<F, T>
    where F: FnOnce(&mut PlutoTxContext) -> T {
        PlutoTx {
            p: self.p.clone(),
            state: PlutoTxState::Pending(body),
        }
    }

    /// Creates a new observer for changes to the shared information.
    pub fn observer(&self) -> Observer<u32> {
        self.p.borrow_mut().observer()
    }
}

impl PlutoReader for Pluto {
    fn get(&self) -> u32 {
        self.p.borrow().val
    }
}

/// A transaction on shared information.
///
/// This struct is a Future that must be polled in order to drive a transaction to completion.
/// The Future will be resolved when the transaction has finished and all observers have seen
/// the resulting changes.
pub struct PlutoTx<F, T> {
    p: PlutoRef,
    state: PlutoTxState<F, T>,
}

enum PlutoTxState<F, T> {
    Empty,
    Pending(F),
    Finalizing(T, Completion),
    Finished(T),
}

impl<F, T> Future for PlutoTx<F, T> where F: FnOnce(&mut PlutoTxContext) -> T {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<T, ()> {
        loop {
            match mem::replace(&mut self.state, PlutoTxState::Empty) {
                PlutoTxState::Empty => panic!("empty"),

                PlutoTxState::Pending(body) => {
                    let mut ctx = PlutoTxContext::open(self.p.clone());

                    let result = body(&mut ctx);

                    if let Some(completion) = ctx.finalize() {
                        self.state = PlutoTxState::Finalizing(result, completion);
                    } else {
                        self.state = PlutoTxState::Finished(result);
                    }
                },

                PlutoTxState::Finalizing(result, mut completion) => {
                    match completion.poll() {
                        Ok(Async::Ready(_)) => {
                            self.state = PlutoTxState::Finished(result);
                        },
                        Ok(Async::NotReady) => {
                            self.state = PlutoTxState::Finalizing(result, completion);
                            return Ok(Async::NotReady);
                        },
                        Err(_) => {
                            return Err(());
                        },
                    }
                },

                PlutoTxState::Finished(result) => {
                    return Ok(Async::Ready(result));
                },
            }
        }
    }
}
