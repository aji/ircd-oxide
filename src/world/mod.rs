use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use futures::Stream;

use time;

use tokio_core::reactor::Handle;

use crdb;
use common::observe::Observable;
use common::observe::Observer;

struct WorldInner {
    db: crdb::CRDB, // TODO: move this out of World

    u_table: crdb::Table<UserSchema>,
    users: HashSet<String>,

    c_table: crdb::Table<ChannelSchema>,
    chans: HashSet<String>,

    m_table: crdb::Table<MembershipSchema>,
    users_for_chan: HashMap<String, HashSet<String>>,
    chans_for_user: HashMap<String, HashSet<String>>,

    events: Observable<WorldEvent>,
}

impl WorldInner {
    fn new() -> WorldInner {
        let mut db = crdb::CRDB::new();

        let u_table = db.create_table("u", UserSchema);
        let c_table = db.create_table("c", ChannelSchema);
        let m_table = db.create_table("m", MembershipSchema);

        WorldInner {
            db: db,

            u_table: u_table,
            users: HashSet::new(),

            c_table: c_table,
            chans: HashSet::new(),

            m_table: m_table,
            users_for_chan: HashMap::new(),
            chans_for_user: HashMap::new(),

            events: Observable::new(),
        }
    }

    fn add_user(&mut self, user: String) -> crdb::Completion {
        let mut tx = self.u_table.open();
        tx.add(user, UserRecord);
        self.db.commit(tx)
    }

    fn add_chan(&mut self, chan: String) -> crdb::Completion {
        let mut tx = self.c_table.open();
        tx.add(chan, ChannelRecord);
        self.db.commit(tx)
    }

    fn join_user(&mut self, chan: String, user: String) -> crdb::Completion {
        let mut tx = self.m_table.open();
        tx.add(format!("{}:{}", user, chan), MembershipRecord::present());
        self.db.commit(tx)
    }

    fn part_user(&mut self, chan: String, user: String) -> crdb::Completion {
        let mut tx = self.m_table.open();
        tx.add(format!("{}:{}", user, chan), MembershipRecord::left());
        self.db.commit(tx)
    }
}

#[derive(Debug)]
pub enum WorldEvent {
    UserJoin(String, String), // chan, user
    UserPart(String, String), // chan, user
}

#[derive(Clone)]
pub struct World {
    inner: Rc<RefCell<WorldInner>>,
}

impl World {
    pub fn new(handle: &Handle) -> World {
        let inner = WorldInner::new();
        let mut world = World { inner: Rc::new(RefCell::new(inner)) };

        world.bind_raw(handle);
        world.bind_u_table(handle);
        world.bind_c_table(handle);
        world.bind_m_table(handle);

        world
    }

    pub fn events(&mut self) -> Observer<WorldEvent> {
        self.inner.borrow_mut().events.observer()
    }

    pub fn add_user(&mut self, user: String) -> crdb::Completion {
        self.inner.borrow_mut().add_user(user)
    }

    pub fn add_chan(&mut self, chan: String) -> crdb::Completion {
        self.inner.borrow_mut().add_chan(chan)
    }

    pub fn join_user(&mut self, chan: String, user: String) -> crdb::Completion {
        self.inner.borrow_mut().join_user(chan, user)
    }

    pub fn part_user(&mut self, chan: String, user: String) -> crdb::Completion {
        self.inner.borrow_mut().part_user(chan, user)
    }

    fn bind_raw(&mut self, handle: &Handle) {
        let updates = self.inner.borrow_mut().db.updates();

        handle.spawn(updates.for_each(|updates| {
            info!("raw updates: {:?}", updates);
            Ok(())
        }));
    }

    fn bind_u_table(&mut self, handle: &Handle) {
        let inner = self.inner.clone();
        let updates = inner.borrow_mut().c_table.updates();

        handle.spawn(updates.for_each(move |updates| {
            info!("u table updates: {:?}", updates);

            let ref mut users = inner.borrow_mut().users;
            for update in updates.updates.iter() {
                users.insert(update.key.clone());
            }

            Ok(())
        }));
    }

    fn bind_c_table(&mut self, handle: &Handle) {
        let inner = self.inner.clone();
        let updates = inner.borrow_mut().c_table.updates();

        handle.spawn(updates.for_each(move |updates| {
            info!("c table updates: {:?}", updates);

            let ref mut chans = inner.borrow_mut().chans;
            for update in updates.updates.iter() {
                chans.insert(update.key.clone());
            }

            Ok(())
        }));
    }

    fn bind_m_table(&mut self, handle: &Handle) {
        let inner = self.inner.clone();
        let updates = inner.borrow_mut().m_table.updates();

        handle.spawn(updates.for_each(move |updates| {
            use self::MembershipStatus::*;
            use self::WorldEvent::*;

            info!("m table updates: {:?}", updates);

            let mut inner_mut = inner.borrow_mut();

            for update in updates.updates.iter() {
                let fields: Vec<&str> = update.key.splitn(2, ':').collect();
                assert_eq!(fields.len(), 2);

                let user = fields[0];
                let chan = fields[1];

                let prev_status = update.prev.as_ref().map(|m| m.status.clone()).unwrap_or(Left);
                let curr_status = update.item.status.clone();

                match (prev_status, curr_status) {
                    (Left, Present) => {
                        inner_mut.users_for_chan
                            .entry(chan.to_string())
                            .or_insert_with(|| HashSet::new())
                            .insert(user.to_string());
                        inner_mut.chans_for_user
                            .entry(user.to_string())
                            .or_insert_with(|| HashSet::new())
                            .insert(chan.to_string());

                        inner_mut.events.put(UserJoin(chan.to_string(), user.to_string()));
                    },

                    (Present, Left) => {
                        inner_mut.users_for_chan
                            .get_mut(chan)
                            .map(|m| m.remove(user));
                        inner_mut.chans_for_user
                            .get_mut(user)
                            .map(|m| m.remove(chan));

                        inner_mut.events.put(UserPart(chan.to_string(), user.to_string()));
                    },

                    _ => { }
                }
            }

            Ok(())
        }));
    }
}

#[derive(Debug, Clone)]
struct UserRecord;

struct UserSchema;

impl crdb::Schema for UserSchema {
    type Item = UserRecord;

    fn decode(&self, _: &crdb::Record) -> UserRecord { UserRecord }
    fn encode(&self, _: &UserRecord) -> crdb::Record { crdb::Record(Vec::new()) }
    fn merge(&self, a: UserRecord, _: UserRecord) -> UserRecord { a }
}

#[derive(Debug, Clone)]
struct ChannelRecord;

struct ChannelSchema;

impl crdb::Schema for ChannelSchema {
    type Item = ChannelRecord;

    fn decode(&self, _: &crdb::Record) -> ChannelRecord { ChannelRecord }
    fn encode(&self, _: &ChannelRecord) -> crdb::Record { crdb::Record(Vec::new()) }
    fn merge(&self, a: ChannelRecord, _: ChannelRecord) -> ChannelRecord { a }
}

#[derive(Debug, Clone)]
struct MembershipRecord {
    since: Timestamp,
    status: MembershipStatus,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum MembershipStatus {
    Present,
    Left,
}

impl MembershipRecord {
    fn with_status(status: MembershipStatus) -> MembershipRecord {
        MembershipRecord { since: Timestamp::now(), status: status }
    }

    fn present() -> MembershipRecord {
        MembershipRecord::with_status(MembershipStatus::Present)
    }

    fn left() -> MembershipRecord {
        MembershipRecord::with_status(MembershipStatus::Left)
    }
}

struct MembershipSchema;

impl crdb::Schema for MembershipSchema {
    type Item = MembershipRecord;

    fn decode(&self, data: &crdb::Record) -> MembershipRecord {
        let spec = String::from_utf8_lossy(&data.0[..]).into_owned();
        let (status, since) = spec.split_at(1);

        MembershipRecord {
            status: match status {
                "P" => MembershipStatus::Present,
                "L" => MembershipStatus::Left,
                _ => panic!("unknown membership status"),
            },
            since: Timestamp::parse(since),
        }
    }

    fn encode(&self, rec: &MembershipRecord) -> crdb::Record {
        let s = format!("{}{}",
            match rec.status {
                MembershipStatus::Present => "P",
                MembershipStatus::Left => "L"
            },
            rec.since.format()
        );

        crdb::Record(s.into_bytes())
    }

    fn merge(&self, a: MembershipRecord, b: MembershipRecord) -> MembershipRecord {
        if a.since > b.since { a } else { b }
    }
}

const TIME_FORMAT: &'static str = "%y%m%d%H%M%S";

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Timestamp(String);

impl Timestamp {
    fn now() -> Timestamp {
        Timestamp(time::strftime(TIME_FORMAT, &time::now_utc()).unwrap())
    }

    fn format(&self) -> &str {
        &self.0[..]
    }

    fn parse(s: &str) -> Timestamp {
        Timestamp(s.to_string())
    }
}
