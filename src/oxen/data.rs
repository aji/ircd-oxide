// oxen/data.rs -- types for Oxen parcels and messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::convert::From;

use util::Sid;
use xenc;
use xenc::FromXenc;

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

impl FromXenc for Parcel {
    fn from_xenc(v: xenc::Value) -> xenc::Result<Parcel> {
        let mut map = try!(v.into_dict().ok_or(xenc::Error));

        let ka = if let Some(ka) = map.remove(b"ka" as &[u8]) {
            Some(try!(ka.into_i64().ok_or(xenc::Error)))
        } else {
            None
        };

        let kk = if let Some(kk) = map.remove(b"kk" as &[u8]) {
            Some(try!(kk.into_i64().ok_or(xenc::Error)))
        } else {
            None
        };

        let body = try!(ParcelBody::from_xenc(&mut map));

        Ok(Parcel {
            ka_rq: ka.map(|v| v as KeepaliveId),
            ka_ok: kk.map(|v| v as KeepaliveId),
            body: body,
        })
    }
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
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<ParcelBody> {
        use self::ParcelBody::*;

        let t = if let Some(t) = map.remove(b"t" as &[u8]) {
            try!(t.into_octets().ok_or(xenc::Error))
        } else {
            return Ok(Missing);
        };

        match &t[..] {
            b"md" => Ok(MsgData(try!(self::MsgData::from_xenc(map)))),
            b"ma" => Ok(MsgAck(try!(self::MsgAck::from_xenc(map)))),
            b"lc" => Ok(LcGossip(try!(self::LcGossip::from_xenc(map)))),
            _ => Err(xenc::Error),
        }
    }

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
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgData> {
        let to: Sid = try!(map
            .remove(b"to" as &[u8])
            .ok_or(xenc::Error)
            .and_then(|v| FromXenc::from_xenc(v))
        );

        let fr: Sid = try!(map
            .remove(b"fr" as &[u8])
            .ok_or(xenc::Error)
            .and_then(|v| FromXenc::from_xenc(v))
        );

        let id = if let Some(id) = map.remove(b"id" as &[u8]) {
            Some(try!(id.into_i64().ok_or(xenc::Error)))
        } else {
            None
        };

        Ok(MsgData {
            to: to,
            fr: fr,
            id: id.map(|v| v as MsgId),
            body: MsgDataBody::Missing,
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"t".to_vec(),  From::from(b"md".to_vec()));
        map.insert(b"to".to_vec(), From::from(self.to));
        map.insert(b"fr".to_vec(), From::from(self.fr));

        if let Some(id) = self.id {
            map.insert(b"id".to_vec(), From::from(id as i64));
        }

        self.body.into_xenc(map);
    }
}

impl MsgAck {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgAck> {
        let to: Sid = try!(map
            .remove(b"to" as &[u8])
            .ok_or(xenc::Error)
            .and_then(|v| FromXenc::from_xenc(v))
        );

        let fr: Sid = try!(map
            .remove(b"fr" as &[u8])
            .ok_or(xenc::Error)
            .and_then(|v| FromXenc::from_xenc(v))
        );

        let id: i64 = try!(map
            .remove(b"id" as &[u8])
            .and_then(|v| v.into_i64())
            .ok_or(xenc::Error)
        );

        Ok(MsgAck {
            to: to,
            fr: fr,
            id: id as MsgId,
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"t".to_vec(),  From::from(b"ma".to_vec()));
        map.insert(b"to".to_vec(), From::from(self.to));
        map.insert(b"fr".to_vec(), From::from(self.fr));
        map.insert(b"id".to_vec(), From::from(self.id as i64));
    }
}

impl LcGossip {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<LcGossip> {
        Err(xenc::Error)
        // TODO
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"t".to_vec(),  From::from(b"lc".to_vec()));
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
