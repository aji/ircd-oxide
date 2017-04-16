//! A futures-based mechanism for broadcasting information.
//!
//! # Observation lifecycle
//!
//! Broadcasts begin with an `Observable`, which represents a series of updates to which an
//! arbitrary number of observers can be attached. When an update is sent, the data is packaged
//! immutably into an `Observation` that is sent to all observers. Updates are received by
//! observers in the same order they were sent.
//!
//! After the update has been queued for delivery to all observers, a `Completion` is returned
//! to the caller. `Completion` is a `Future` that is resolved when the observation has been
//! fully consumed by all observers. Consuming an update, in this case, does *not* simply mean
//! pulling the update off the queue. Rather, an update is only fully consumed when references
//! to it no longer exist and the data has been dropped completely or cloned elsewhere for
//! deeper asynchronous handling.
//!
//! In general, callers are free to do what they want with the returned `Completion`, including
//! ignoring it completely. The `Completion` is simply for the caller's benefit, as there may
//! be nearby code that is processing updates and the caller wants some additional code that is
//! only run when the associated observer has finished processing the update.
//!
//! # Example
//!
//! ```rust,no_run
//! # extern crate tokio_core;
//! # extern crate futures;
//! # extern crate oxide;
//! #
//! # use oxide::common::observe::Observable;
//! # use tokio_core::reactor::{Core, Handle};
//! # use futures::{Future, Stream};
//! #
//! # fn main() {
//! #
//! # let reactor = Core::new().unwrap();
//! # let handle = reactor.handle();
//! #
//! // let handle: Handle = ...
//!
//! let mut updates: Observable<&'static str> = Observable::new();
//!
//! handle.spawn(updates.observer().for_each(|obs| {
//!     println!("got data: {}", obs);
//!     Ok(())
//! }));
//!
//! // Send an update, printing a message when the Completion finishes
//! handle.spawn(updates.put("hello").then(|_| {
//!     println!("update was received!");
//!     Ok(())
//! }));
//!
//! // Send an update, discarding the Completion
//! updates.put("world");
//! #
//! # }
//! ```

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

/// An observable channel, to which updates can be sent.
///
/// Updates must be of the type `T`. Updates will be seen by all observers in the same order
/// that they are submitted. See the module-level documentation for more information.
pub struct Observable<T> {
    dispatch: Vec<Weak<DispatchCell<T>>>,
}

/// A `Stream` of updates from a given observable.
pub struct Observer<T> {
    dispatch: Rc<DispatchCell<T>>,
}

struct Shared {
    parked: Option<task::Task>,
}

/// A `Future` created on submission of an update. The future will complete when all
/// observers have dropped the observation. See the module-level documentation for more
/// informatmion.
pub struct Completion {
    shared: Rc<RefCell<Shared>>,
    signal: Weak<()>,
}

/// An update from an `Observable`.
///
/// The contained data cannot be moved out of the `Observation`. If storing data from an update
/// is important, that data should be cloned. That is, the `Observation` should not be kept past
/// when it is needed. See the [`Observable`](struct.Observable.html) for more information.
///
/// Critically, an `Observation` should be dropped as soon as the data is no longer relevant.
/// The existence of an `Observation` for a particular update, anywhere in memory, indicates that
/// the update is still being processed, and blocks resolution of the `Completion` which is
/// returned to the caller as a result.
pub struct Observation<T> {
    shared: Weak<RefCell<Shared>>,
    signal: Rc<()>,
    data: Rc<T>,
}

impl<T: fmt::Debug> Observable<T> {
    /// Creates a new `Observable`
    pub fn new() -> Observable<T> {
        Observable { dispatch: Vec::new() }
    }

    /// Broadcasts an item to all observers. The returned `Completion` will be resolved when
    /// all observers have dropped the resulting `Observation`.
    pub fn put(&mut self, data: T) -> Completion {
        let shared_inner = Shared { parked: None };
        let shared = Rc::new(RefCell::new(shared_inner));

        let observation = Observation {
            shared: Rc::downgrade(&shared),
            signal: Rc::new(()),
            data: Rc::new(data),
        };

        let completion = Completion {
            shared: shared,
            signal: Rc::downgrade(&observation.signal),
        };

        self.dispatch(observation);

        completion
    }

    /// Creates a new observer for this update stream. The Observer will immediately begin
    /// receiving updates.
    pub fn observer(&mut self) -> Observer<T> {
        let dispatch_inner = Dispatch { pending: VecDeque::new(), parked: None };
        let dispatch = Rc::new(RefCell::new(dispatch_inner));

        self.dispatch.push(Rc::downgrade(&dispatch));

        Observer { dispatch: dispatch }
    }

    fn dispatch(&mut self, obs: Observation<T>) {
        // if this becomes a bottleneck, it can be made better by iterating over
        // indices and using swap_remove to delete dropped weak pointers

        debug!("dispatching observation: {:?}", obs);

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

impl<T> Drop for Observable<T> {
    fn drop(&mut self) {
        for r in self.dispatch.drain(..) {
            if let Some(dispatch) = r.upgrade() {
                dispatch.borrow_mut().parked.take().map(|t| t.unpark());
            }
        }
    }
}

impl Future for Completion {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let None = self.signal.upgrade() {
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
        let weak_count = Rc::weak_count(&self.dispatch);
        let mut dispatch = self.dispatch.borrow_mut();

        if let Some(obs) = dispatch.pending.pop_front() {
            Ok(Async::Ready(Some(obs)))
        } else if weak_count == 0 {
            Ok(Async::Ready(None))
        } else {
            dispatch.parked = Some(task::park());
            Ok(Async::NotReady)
        }
    }
}

impl<T> Observation<T> {
    /// If the update needs to be kept around for a longer period of time, then the
    /// `Observation` can be converted directly into the underlying `Rc` wrapping the data.
    /// This is still considered to be "consuming" the `Observation`, for the purpose of
    /// notifying the attached `Completion`.
    pub fn into_inner(self) -> Rc<T> {
        self.data.clone()
    }
}

impl<T> Clone for Observation<T> {
    fn clone(&self) -> Observation<T> {
        Observation {
            shared: self.shared.clone(),
            signal: self.signal.clone(),
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
        write!(f, "Observation({:?})", &*self.data)
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
