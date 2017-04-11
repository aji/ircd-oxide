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

    fn send_update(&mut self) -> Completion<u32> {
        self.observable.put(self.val)
    }
}

pub trait PlutoReader {
    fn get(&self) -> u32;
}

pub trait PlutoWriter {
    fn set(&mut self, x: u32) -> ();
}

pub struct PlutoTxContext {
    p: PlutoRef,
    val_changed: bool
}

impl PlutoTxContext {
    fn open(p: PlutoRef) -> PlutoTxContext {
        PlutoTxContext { p: p, val_changed: false }
    }

    fn finalize(self) -> Option<Completion<u32>> {
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

#[derive(Clone)]
pub struct Pluto {
    p: PlutoRef
}

impl Pluto {
    pub fn new() -> Pluto {
        Pluto { p: PlutoCore::new().into_ref() }
    }

    pub fn tx<F, T>(&self, body: F) -> PlutoTx<F, T>
    where F: FnOnce(&mut PlutoTxContext) -> T {
        PlutoTx {
            p: self.p.clone(),
            state: PlutoTxState::Pending(body),
        }
    }

    pub fn observer(&self) -> Observer<u32> {
        self.p.borrow_mut().observer()
    }
}

impl PlutoReader for Pluto {
    fn get(&self) -> u32 {
        self.p.borrow().val
    }
}

pub struct PlutoTx<F, T> {
    p: PlutoRef,
    state: PlutoTxState<F, T>,
}

enum PlutoTxState<F, T> {
    Empty,
    Pending(F),
    Finalizing(T, Completion<u32>),
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
