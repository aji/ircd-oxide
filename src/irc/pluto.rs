use std::rc::Rc;
use std::cell::RefCell;

use futures;
use futures::Async;
use futures::Future;
use futures::sync::mpsc;

struct PlutoCore {
    val: u32,
    observers: Vec<mpsc::UnboundedSender<u32>>,
}

type PlutoRef = Rc<RefCell<PlutoCore>>;

impl PlutoCore {
    fn new() -> PlutoCore {
        PlutoCore { val: 0, observers: Vec::new() }
    }

    fn into_ref(self) -> PlutoRef {
        Rc::new(RefCell::new(self))
    }

    fn add_observer(&mut self) -> mpsc::UnboundedReceiver<u32> {
        let (sender, receiver) = mpsc::unbounded();
        self.observers.push(sender);
        receiver
    }

    fn send_update(&mut self) {
        for obs in self.observers.iter_mut() {
            obs.send(self.val);
        }
    }
}

pub struct PlutoTxContext {
    p: PlutoRef,
    val_changed: bool
}

impl PlutoTxContext {
    fn open(p: PlutoRef) -> PlutoTxContext {
        PlutoTxContext { p: p, val_changed: false }
    }

    fn finalize(self) {
        if self.val_changed {
            self.p.borrow_mut().send_update();
        }
    }

    pub fn get(&self) -> u32 {
        self.p.borrow().val
    }

    pub fn set(&mut self, x: u32) {
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

    pub fn tx<F, T>(&self, body: F) -> PlutoTx<F>
    where F: FnOnce(&mut PlutoTxContext) -> T {
        PlutoTx {
            p: self.p.clone(),
            body: Some(body),
        }
    }

    pub fn observer(&self) -> mpsc::UnboundedReceiver<u32> {
        self.p.borrow_mut().add_observer()
    }
}

pub struct PlutoTx<F> {
    p: PlutoRef,
    body: Option<F>,
}

impl<F> PlutoTx<F> {
    fn take_body(&mut self) -> F {
        self.body.take().expect("cannot poll PlutoTx more than once")
    }
}

impl<F, T> Future for PlutoTx<F> where F: FnOnce(&mut PlutoTxContext) -> T {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> futures::Poll<T, ()> {
        let body = self.take_body();
        let mut ctx = PlutoTxContext::open(self.p.clone());
        let result = body(&mut ctx);
        ctx.finalize();
        Ok(Async::Ready(result))
    }
}
