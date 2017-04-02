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

trait PlutoRefHolder {
    fn pluto_ref(&self) -> &PlutoRef;
}

pub trait PlutoReader {
    fn get(&self) -> u32;
}

pub trait PlutoWriter {
    fn set(&mut self, x: u32) -> ();
}

impl<T> PlutoReader for T where T: PlutoRefHolder {
    fn get(&self) -> u32 {
        self.pluto_ref().borrow().val
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
}

impl PlutoRefHolder for PlutoTxContext {
    fn pluto_ref(&self) -> &PlutoRef { &self.p }
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

impl PlutoRefHolder for Pluto {
    fn pluto_ref(&self) -> &PlutoRef { &self.p }
}

pub struct PlutoTx<F> {
    p: PlutoRef,
    body: Option<F>,
}

impl<F, T> Future for PlutoTx<F> where F: FnOnce(&mut PlutoTxContext) -> T {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> futures::Poll<T, ()> {
        let body = self.body.take().expect("cannot poll PlutoTx more than once");
        let mut ctx = PlutoTxContext::open(self.p.clone());
        let result = body(&mut ctx);
        ctx.finalize();
        Ok(Async::Ready(result))
    }
}
