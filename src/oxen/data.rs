// oxen/data.rs -- types for Oxen parcels and messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::convert::From;

use util::Sid;
use xenc;

pub type KeepaliveId = u32;
pub type MsgId = u32;
pub type SeqNum = u32;

pub struct Parcel {
    ka_rq: Option<KeepaliveId>,
    ka_ok: Option<KeepaliveId>,
    body: ParcelBody,
}

pub enum ParcelBody {
    Missing,
    MsgData(MsgData),
    MsgAck(MsgAck),
    LcGossip(LcGossip),
}

pub struct MsgData {
    to: Sid,
    fr: Sid,
    id: Option<MsgId>,
    body: MsgDataBody,
}

pub struct MsgAck {
    to: Sid,
    fr: Sid,
    id: MsgId,
}

pub struct LcGossip {
    _rows: HashMap<Sid, Vec<f64>>,
    _cols: Vec<Sid>,
}

pub enum MsgDataBody {
    Missing,
    MsgSync(MsgSync),
    MsgFinal(MsgFinal),
    MsgBrd(MsgBrd),
    MsgOne(MsgOne),
}

pub struct MsgSync {
    brd: SeqNum,
    one: SeqNum,
}

pub struct MsgFinal {
    brd: SeqNum,
    one: SeqNum,
}

pub struct MsgBrd {
    seq: SeqNum,
    data: Vec<u8>,
}

pub struct MsgOne {
    seq: SeqNum,
    data: Vec<u8>,
}

impl From<Parcel> for xenc::Value {
    fn from(p: Parcel) -> xenc::Value {
        let mut map = HashMap::new();

        if let Some(ka) = p.ka_rq {
            map.insert(b"ka".to_vec(), From::from(ka as i64));
        }
        if let Some(kk) = p.ka_ok {
            map.insert(b"kk".to_vec(), From::from(kk as i64));
        }

        p.body.into_xenc(&mut map);

        xenc::Value::Dict(map)
    }
}

impl ParcelBody {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        use self::ParcelBody::*;

        match self {
            Missing         => (),
            MsgData(md)     => md.into_xenc(map),
            MsgAck(ma)      => ma.into_xenc(map),
            LcGossip(lc)    => lc.into_xenc(map),
        }
    }
}

impl MsgData {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"to".to_vec(), From::from(self.to));
        map.insert(b"fr".to_vec(), From::from(self.fr));

        if let Some(id) = self.id {
            map.insert(b"id".to_vec(), From::from(id as i64));
        }

        self.body.into_xenc(map);
    }
}

impl MsgAck {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"to".to_vec(), From::from(self.to));
        map.insert(b"fr".to_vec(), From::from(self.fr));
        map.insert(b"id".to_vec(), From::from(self.id as i64));
    }
}

impl LcGossip {
    fn into_xenc(self, _map: &mut HashMap<Vec<u8>, xenc::Value>) {
        // TODO
    }
}

impl MsgDataBody {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        use self::MsgDataBody::*;

        match self {
            Missing         => (),
            MsgSync(syn)    => syn.into_xenc(map),
            MsgFinal(fin)   => fin.into_xenc(map),
            MsgBrd(brd)     => brd.into_xenc(map),
            MsgOne(one)     => one.into_xenc(map),
        }
    }
}

impl MsgSync {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"s".to_vec()));
        map.insert(b"b".to_vec(), From::from(self.brd as i64));
        map.insert(b"1".to_vec(), From::from(self.one as i64));
    }
}

impl MsgFinal {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"f".to_vec()));
        map.insert(b"b".to_vec(), From::from(self.brd as i64));
        map.insert(b"1".to_vec(), From::from(self.one as i64));
    }
}

impl MsgBrd {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"b".to_vec()));
        map.insert(b"s".to_vec(), From::from(self.seq as i64));
        map.insert(b"d".to_vec(), From::from(self.data));
    }
}

impl MsgOne {
    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"o".to_vec()));
        map.insert(b"s".to_vec(), From::from(self.seq as i64));
        map.insert(b"d".to_vec(), From::from(self.data));
    }
}
