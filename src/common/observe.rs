use std::borrow;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::ops;
use std::rc::Rc;
use std::rc::Weak;

use futures::Future;
use futures::Async;
use futures::Poll;
use futures::Stream;
use futures::task;

struct Dispatch<T> {
    pending: VecDeque<Observation<T>>,
    parked: Option<task::Task>,
}

type DispatchCell<T> = RefCell<Dispatch<T>>;

pub struct Observable<T> {
    dispatch: Vec<Weak<DispatchCell<T>>>,
}

pub struct Observer<T> {
    dispatch: Rc<DispatchCell<T>>,
}

struct Shared {
    parked: Option<task::Task>,
}

pub struct Completion<T> {
    shared: Rc<RefCell<Shared>>,
    data: Weak<T>,
}

pub struct Observation<T> {
    shared: Weak<RefCell<Shared>>,
    data: Rc<T>,
}

impl<T> Observable<T> {
    pub fn new() -> Observable<T> {
        Observable { dispatch: Vec::new() }
    }

    pub fn put(&mut self, data: T) -> Completion<T> {
        let shared_inner = Shared { parked: None };
        let shared = Rc::new(RefCell::new(shared_inner));

        let observation = Observation {
            shared: Rc::downgrade(&shared),
            data: Rc::new(data),
        };

        let completion = Completion {
            shared: shared,
            data: Rc::downgrade(&observation.data),
        };

        self.dispatch(observation);

        completion
    }

    pub fn observer(&mut self) -> Observer<T> {
        let dispatch_inner = Dispatch { pending: VecDeque::new(), parked: None };
        let dispatch = Rc::new(RefCell::new(dispatch_inner));

        self.dispatch.push(Rc::downgrade(&dispatch));

        Observer { dispatch: dispatch }
    }

    fn dispatch(&mut self, obs: Observation<T>) {
        // if this becomes a bottleneck, it can be made better by iterating over
        // indices and using swap_remove to delete dropped weak pointers

        let processed = self.dispatch
            .drain(..)
            .filter_map(|r| r.upgrade())
            .map(|dispatch| {
                let mut inner = dispatch.borrow_mut();
                inner.pending.push_back(obs.clone());
                inner.parked.take().map(|t| t.unpark());
                drop(inner);
                Rc::downgrade(&dispatch)
            })
            .collect();

        self.dispatch = processed;
    }
}

impl<T> Future for Completion<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let None = self.data.upgrade() {
            Ok(Async::Ready(()))
        } else {
            self.shared.borrow_mut().parked = Some(task::park());
            Ok(Async::NotReady)
        }
    }
}

impl<T> Stream for Observer<T> {
    type Item = Observation<T>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Observation<T>>, ()> {
        let mut dispatch = self.dispatch.borrow_mut();
        if let Some(obs) = dispatch.pending.pop_back() {
            Ok(Async::Ready(Some(obs)))
        } else {
            dispatch.parked = Some(task::park());
            Ok(Async::NotReady)
        }
    }
}

impl<T> Clone for Observation<T> {
    fn clone(&self) -> Observation<T> {
        Observation {
            shared: self.shared.clone(),
            data: self.data.clone()
        }
    }
}

impl<T> Drop for Observation<T> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.borrow_mut().parked.take().map(|t| t.unpark());
        }
    }
}

impl<T> fmt::Display for Observation<T> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&*self.data, f)
    }
}

impl<T> fmt::Debug for Observation<T> where T: fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Observation("));
        try!(fmt::Debug::fmt(&*self.data, f));
        try!(write!(f, ")"));
        Ok(())
    }
}

impl<T> ops::Deref for Observation<T> {
    type Target = T;
    fn deref(&self) -> &T { &*self.data }
}

impl<T> borrow::Borrow<T> for Observation<T> {
    fn borrow(&self) -> &T { &*self.data }
}

impl<T> AsRef<T> for Observation<T> {
    fn as_ref(&self) -> &T { &*self.data }
}
