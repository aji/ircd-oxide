use std::collections::HashMap;
use std::rc::Rc;

use futures::Async;
use futures::Future;
use futures::Stream;

use tokio_core::reactor::Core;

use super::*;

struct Min;

struct Max;

impl Schema for Min {
    type Item = u8;
    fn encode(&self, item: &u8) -> Record { Record(Vec::from(&[*item][..])) }
    fn decode(&self, data: &Record) -> u8 { data.0[0] }
    fn merge(&self, a: u8, b: u8) -> u8 { if a < b { a } else { b } }
}

impl Schema for Max {
    type Item = u8;
    fn encode(&self, item: &u8) -> Record { Record(Vec::from(&[*item][..])) }
    fn decode(&self, data: &Record) -> u8 { data.0[0] }
    fn merge(&self, a: u8, b: u8) -> u8 { if a > b { a } else { b } }
}

struct Finish {
    raw_updates: Vec<Rc<RawUpdates>>,
    min_updates: Vec<Rc<Updates<Min>>>,
    max_updates: Vec<Rc<Updates<Max>>>,
    min_finish: HashMap<String, u8>,
    max_finish: HashMap<String, u8>,
}

fn with_test_crdb<F>(body: F) -> Finish
    where F: FnOnce(&mut CRDB, &mut Table<Min>, &mut Table<Max>)
{
    let mut db = CRDB::new();
    let mut min = db.create_table("min", Min);
    let mut max = db.create_table("max", Max);

    let raw_observer = db.updates().map(|obs| obs.into_inner()).collect();
    let min_observer = min.updates().map(|obs| obs.into_inner()).collect();
    let max_observer = max.updates().map(|obs| obs.into_inner()).collect();

    // body creates some transactions, does some stuff, etc
    body(&mut db, &mut min, &mut max);

    // drop now to allow observerables to terminate
    let min_finish = min.snapshot();
    let max_finish = max.snapshot();
    drop(db);

    Finish {
        raw_updates: raw_observer.wait().expect("raw_observer"),
        min_updates: min_observer.wait().expect("min_observer"),
        max_updates: max_observer.wait().expect("max_observer"),
        min_finish: min_finish,
        max_finish: max_finish,
    }
}

fn assert_raw_update(r: &RawUpdate, table: &str, key: &str, prev: Option<u8>, item: u8) {
    assert_eq!(r.table, table);
    assert_eq!(r.key, key);
    assert_eq!(r.item.0[0], item);
    assert_eq!(r.prev.as_ref().map(|r| r.0[0]), prev);
}

fn assert_update<S: Schema<Item=u8>>(r: &Update<S>, key: &str, prev: Option<u8>, item: u8) {
    assert_eq!(r.key, key);
    assert_eq!(r.prev, prev);
    assert_eq!(r.item, item);
}

#[test]
fn simply_commit() {
    let fin = with_test_crdb(|db, min, _max| {
        let mut tx = min.open();
        tx.add("a".to_string(), 10);
        tx.add("b".to_string(), 15);
        db.commit(tx);
    });

    assert_eq!(fin.raw_updates.len(), 1);
    assert_eq!(fin.raw_updates[0].updates.len(), 2);

    assert_eq!(fin.min_updates.len(), 1);
    assert_eq!(fin.min_updates[0].updates.len(), 2);
    assert_eq!(fin.min_finish.len(), 2);
    assert_eq!(fin.min_finish.get("a"), Some(&10));
    assert_eq!(fin.min_finish.get("b"), Some(&15));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn simple_multiple_commits() {
    let fin = with_test_crdb(|db, min, max| {
        {
            let mut tx = min.open();
            tx.add("a".to_string(), 10);
            tx.add("b".to_string(), 15);
            db.commit(tx);
        }

        {
            let mut tx = max.open();
            tx.add("c".to_string(), 11);
            tx.add("d".to_string(), 16);
            db.commit(tx);
        }
    });

    assert_eq!(fin.raw_updates.len(), 2);
    assert_eq!(fin.raw_updates[0].updates.len(), 2);
    assert_eq!(fin.raw_updates[1].updates.len(), 2);

    assert_eq!(fin.min_updates.len(), 1);
    assert_eq!(fin.min_updates[0].updates.len(), 2);
    assert_eq!(fin.min_finish.len(), 2);
    assert_eq!(fin.min_finish.get("a"), Some(&10));
    assert_eq!(fin.min_finish.get("b"), Some(&15));

    assert_eq!(fin.max_updates.len(), 1);
    assert_eq!(fin.max_updates[0].updates.len(), 2);
    assert_eq!(fin.max_finish.len(), 2);
    assert_eq!(fin.max_finish.get("c"), Some(&11));
    assert_eq!(fin.max_finish.get("d"), Some(&16));
}

#[test]
fn two_commits_to_same_entry() {
    let fin = with_test_crdb(|db, min, _max| {
        {
            let mut tx = min.open();
            tx.add("a".to_string(), 10);
            db.commit(tx);
        }

        {
            let mut tx = min.open();
            tx.add("a".to_string(), 5);
            db.commit(tx);
        }
    });

    assert_eq!(fin.raw_updates.len(), 2);
    assert_eq!(fin.raw_updates[0].updates.len(), 1);
    assert_eq!(fin.raw_updates[1].updates.len(), 1);
    assert_raw_update(&fin.raw_updates[0].updates[0], "min", "a", None, 10);
    assert_raw_update(&fin.raw_updates[1].updates[0], "min", "a", Some(10), 5);

    assert_eq!(fin.min_updates.len(), 2);
    assert_eq!(fin.min_updates[0].updates.len(), 1);
    assert_eq!(fin.min_updates[1].updates.len(), 1);
    assert_update(&fin.min_updates[0].updates[0], "a", None, 10);
    assert_update(&fin.min_updates[1].updates[0], "a", Some(10), 5);
    assert_eq!(fin.min_finish.len(), 1);
    assert_eq!(fin.min_finish.get("a"), Some(&5));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn two_commits_to_same_entry_loser() {
    let fin = with_test_crdb(|db, min, _max| {
        {
            let mut tx = min.open();
            tx.add("a".to_string(), 10);
            db.commit(tx);
        }

        {
            let mut tx = min.open();
            tx.add("a".to_string(), 11);
            db.commit(tx);
        }
    });

    assert_eq!(fin.raw_updates.len(), 2);
    assert_eq!(fin.raw_updates[0].updates.len(), 1);
    assert_eq!(fin.raw_updates[1].updates.len(), 1);
    assert_raw_update(&fin.raw_updates[0].updates[0], "min", "a", None, 10);
    assert_raw_update(&fin.raw_updates[1].updates[0], "min", "a", Some(10), 10);

    assert_eq!(fin.min_updates.len(), 2);
    assert_eq!(fin.min_updates[0].updates.len(), 1);
    assert_eq!(fin.min_updates[1].updates.len(), 1);
    assert_update(&fin.min_updates[0].updates[0], "a", None, 10);
    assert_update(&fin.min_updates[1].updates[0], "a", Some(10), 10);
    assert_eq!(fin.min_finish.len(), 1);
    assert_eq!(fin.min_finish.get("a"), Some(&10));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn see_own_writes_and_coalesce() {
    let fin = with_test_crdb(|db, min, _max| {
        let mut tx = min.open();

        assert_eq!(tx.get("a"), None);

        tx.add("a".to_string(), 10);
        assert_eq!(tx.get("a"), Some(10));

        tx.add("a".to_string(), 5);
        assert_eq!(tx.get("a"), Some(5));

        tx.add("a".to_string(), 9);
        assert_eq!(tx.get("a"), Some(5));

        db.commit(tx);
    });

    assert_eq!(fin.raw_updates.len(), 1);
    assert_eq!(fin.raw_updates[0].updates.len(), 1);
    assert_raw_update(&fin.raw_updates[0].updates[0], "min", "a", None, 5);

    assert_eq!(fin.min_updates.len(), 1);
    assert_eq!(fin.min_updates[0].updates.len(), 1);
    assert_update(&fin.min_updates[0].updates[0], "a", None, 5);
    assert_eq!(fin.min_finish.len(), 1);
    assert_eq!(fin.min_finish.get("a"), Some(&5));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn see_own_writes_and_coalesce_2() {
    let fin = with_test_crdb(|db, min, _max| {
        {
            let mut tx = min.open();

            assert_eq!(tx.get("a"), None);

            tx.add("a".to_string(), 10);
            assert_eq!(tx.get("a"), Some(10));

            db.commit(tx);
        }

        {
            let mut tx = min.open();

            assert_eq!(tx.get("a"), Some(10));

            tx.add("a".to_string(), 5);
            assert_eq!(tx.get("a"), Some(5));

            tx.add("a".to_string(), 9);
            assert_eq!(tx.get("a"), Some(5));

            db.commit(tx);
        }
    });

    assert_eq!(fin.raw_updates.len(), 2);
    assert_eq!(fin.raw_updates[0].updates.len(), 1);
    assert_eq!(fin.raw_updates[1].updates.len(), 1);
    assert_raw_update(&fin.raw_updates[0].updates[0], "min", "a", None, 10);
    assert_raw_update(&fin.raw_updates[1].updates[0], "min", "a", Some(10), 5);

    assert_eq!(fin.min_updates.len(), 2);
    assert_eq!(fin.min_updates[0].updates.len(), 1);
    assert_eq!(fin.min_updates[1].updates.len(), 1);
    assert_update(&fin.min_updates[0].updates[0], "a", None, 10);
    assert_update(&fin.min_updates[1].updates[0], "a", Some(10), 5);
    assert_eq!(fin.min_finish.len(), 1);
    assert_eq!(fin.min_finish.get("a"), Some(&5));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn raw_transaction() {
    let fin = with_test_crdb(|db, _min, _max| {
        {
            let mut tx = RawTransaction::new();
            tx.add("min".to_string(), "a".to_string(), Min.encode(&12));
            tx.add("min".to_string(), "a".to_string(), Min.encode(&10));
            db.commit_raw(tx);
        }

        {
            let mut tx = RawTransaction::new();
            tx.add("min".to_string(), "a".to_string(), Min.encode(&5));
            tx.add("min".to_string(), "a".to_string(), Min.encode(&9));
            db.commit_raw(tx);
        }
    });

    assert_eq!(fin.raw_updates.len(), 2);
    assert_eq!(fin.raw_updates[0].updates.len(), 1);
    assert_eq!(fin.raw_updates[1].updates.len(), 1);
    assert_raw_update(&fin.raw_updates[0].updates[0], "min", "a", None, 10);
    assert_raw_update(&fin.raw_updates[1].updates[0], "min", "a", Some(10), 5);

    assert_eq!(fin.min_updates.len(), 2);
    assert_eq!(fin.min_updates[0].updates.len(), 1);
    assert_eq!(fin.min_updates[1].updates.len(), 1);
    assert_update(&fin.min_updates[0].updates[0], "a", None, 10);
    assert_update(&fin.min_updates[1].updates[0], "a", Some(10), 5);
    assert_eq!(fin.min_finish.len(), 1);
    assert_eq!(fin.min_finish.get("a"), Some(&5));

    assert_eq!(fin.max_updates.len(), 0);
    assert_eq!(fin.max_finish.len(), 0);
}

#[test]
fn test_completion() {
    use std::rc::Rc;
    use std::cell::RefCell;

    let mut db = CRDB::new();
    let mut min = db.create_table("min", Min);

    let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let raw_order = order.clone();
    let raw_updates = db.updates().for_each(move |_| {
        raw_order.borrow_mut().push("raw update");
        Ok(())
    });

    let min_order = order.clone();
    let min_updates = db.updates().for_each(move |_| {
        min_order.borrow_mut().push("min update");
        Ok(())
    });

    let completion_order = order.clone();
    let completion = {
        let mut tx = min.open();
        tx.add("a".to_string(), 10);
        db.commit(tx)
    }.and_then(move |_| {
        completion_order.borrow_mut().push("completion");
        Ok(())
    });

    let mut core = Core::new().expect("tokio core");
    let mut handle = core.handle();
    handle.spawn(raw_updates);
    handle.spawn(min_updates);
    core.run(completion).expect("completion");

    let order_data = order.borrow();
    assert_eq!(order_data.len(), 3);
    assert_eq!(order_data[2], "completion");
    assert!(order_data[0] != order_data[1]);
    assert!(order_data[0] == "raw update" || order_data[0] == "min update");
    assert!(order_data[1] == "raw update" || order_data[1] == "min update");
}
