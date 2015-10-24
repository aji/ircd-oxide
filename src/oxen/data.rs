// oxen/data.rs -- types for Oxen parcels and messages
// Copyright (C) 2015 Alex Iadicicco
//
// This file is part of ircd-oxide and is protected under the terms contained in
// the COPYING file in the project root.

use std::collections::HashMap;
use std::convert::From;
use time::Timespec;

use util::Sid;
use xenc;
use xenc::FromXenc;

pub type KeepaliveId = u32;
pub type MsgId = u32;
pub type SeqNum = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parcel {
    ka_rq: Option<KeepaliveId>,
    ka_ok: Option<KeepaliveId>,
    body: ParcelBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParcelBody {
    Missing,
    MsgData(MsgData),
    MsgAck(MsgAck),
    LcGossip(LcGossip),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgData {
    to: Sid,
    fr: Sid,
    id: Option<MsgId>,
    body: MsgDataBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgAck {
    to: Sid,
    fr: Sid,
    id: MsgId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcGossip {
    rows: HashMap<Sid, Vec<Timespec>>,
    cols: Vec<Sid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgDataBody {
    Missing,
    MsgSync(MsgSync),
    MsgFinal(MsgFinal),
    MsgBrd(MsgBrd),
    MsgOne(MsgOne),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgSync {
    brd: SeqNum,
    one: SeqNum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgFinal {
    brd: SeqNum,
    one: SeqNum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgBrd {
    seq: SeqNum,
    data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

        let pt = if let Some(t) = map.remove(b"pt" as &[u8]) {
            try!(t.into_octets().ok_or(xenc::Error))
        } else {
            return Ok(Missing);
        };

        match &pt[..] {
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
            body: try!(MsgDataBody::from_xenc(map)),
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"pt".to_vec(), From::from(b"md".to_vec()));
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
        map.insert(b"pt".to_vec(), From::from(b"ma".to_vec()));
        map.insert(b"to".to_vec(), From::from(self.to));
        map.insert(b"fr".to_vec(), From::from(self.fr));
        map.insert(b"id".to_vec(), From::from(self.id as i64));
    }
}

impl LcGossip {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<LcGossip> {
        let lc = {
            let lc_xenc = try!(map
                .remove(b"lc" as &[u8])
                .and_then(|lc| lc.into_dict())
                .ok_or(xenc::Error)
            );
            let mut lc = HashMap::new();
            for (k, v) in lc_xenc.into_iter() {
                let sid = Sid::from(&k[..]);

                let row = {
                    let row_xenc = try!(v.into_list().ok_or(xenc::Error));
                    let mut row = Vec::new();
                    for v in row_xenc.into_iter() {
                        row.push(try!(v.into_time().ok_or(xenc::Error)));
                    }
                    row
                };

                lc.insert(sid, row);
            }
            lc
        };

        let p = {
            let p_xenc = try!(map
                .remove(b"p" as &[u8])
                .and_then(|p| p.into_list())
                .ok_or(xenc::Error)
            );
            let mut p = Vec::new();
            for v in p_xenc.into_iter() {
                p.push(try!(v
                    .into_octets()
                    .map(|s| Sid::from(&s[..]))
                    .ok_or(xenc::Error)
                ))
            }
            p
        };

        Ok(LcGossip {
            rows: lc,
            cols: p
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"pt".to_vec(), From::from(b"lc".to_vec()));

        map.insert(b"lc".to_vec(), xenc::Value::Dict(
            self.rows.into_iter()
                .map(|(k, row)| (
                    From::from(k),
                    From::from(row.into_iter()
                        .map(|v| From::from(v))
                        .collect::<Vec<xenc::Value>>()
                    )
                ))
                .collect()
        ));

        map.insert(b"p".to_vec(), xenc::Value::List(
            self.cols.into_iter()
                .map(|sid| From::from(sid))
                .collect()
        ));
    }
}

impl MsgDataBody {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgDataBody> {
        use self::MsgDataBody::*;

        let m = if let Some(m) = map.remove(b"m" as &[u8]) {
            try!(m.into_octets().ok_or(xenc::Error))
        } else {
            return Ok(Missing);
        };

        match &m[..] {
            b"s" => Ok(MsgSync(try!(self::MsgSync::from_xenc(map)))),
            b"f" => Ok(MsgFinal(try!(self::MsgFinal::from_xenc(map)))),
            b"b" => Ok(MsgBrd(try!(self::MsgBrd::from_xenc(map)))),
            b"1" => Ok(MsgOne(try!(self::MsgOne::from_xenc(map)))),
            _ => Err(xenc::Error),
        }
    }

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
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgSync> {
        Ok(MsgSync {
            brd: try!(map
                    .remove(b"b" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
            one: try!(map
                    .remove(b"1" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"s".to_vec()));
        map.insert(b"b".to_vec(), From::from(self.brd as i64));
        map.insert(b"1".to_vec(), From::from(self.one as i64));
    }
}

impl MsgFinal {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgFinal> {
        Ok(MsgFinal {
            brd: try!(map
                    .remove(b"b" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
            one: try!(map
                    .remove(b"1" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"f".to_vec()));
        map.insert(b"b".to_vec(), From::from(self.brd as i64));
        map.insert(b"1".to_vec(), From::from(self.one as i64));
    }
}

impl MsgBrd {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgBrd> {
        Ok(MsgBrd {
            seq: try!(map
                    .remove(b"s" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
            data: try!(map
                    .remove(b"d" as &[u8])
                    .and_then(|v| v.into_octets())
                    .ok_or(xenc::Error)),
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"b".to_vec()));
        map.insert(b"s".to_vec(), From::from(self.seq as i64));
        map.insert(b"d".to_vec(), From::from(self.data));
    }
}

impl MsgOne {
    fn from_xenc(map: &mut HashMap<Vec<u8>, xenc::Value>)
    -> xenc::Result<MsgOne> {
        Ok(MsgOne {
            seq: try!(map
                    .remove(b"s" as &[u8])
                    .and_then(|v| v.into_i64())
                    .map(|v| v as MsgId)
                    .ok_or(xenc::Error)),
            data: try!(map
                    .remove(b"d" as &[u8])
                    .and_then(|v| v.into_octets())
                    .ok_or(xenc::Error)),
        })
    }

    fn into_xenc(self, map: &mut HashMap<Vec<u8>, xenc::Value>) {
        map.insert(b"m".to_vec(), From::from(b"1".to_vec()));
        map.insert(b"s".to_vec(), From::from(self.seq as i64));
        map.insert(b"d".to_vec(), From::from(self.data));
    }
}

#[cfg(test)]
fn codec(p: Parcel) -> bool {
    match Parcel::from_xenc(From::from(p.clone())) {
        Ok(q) => p == q,
        Err(_) => false,
    }
}

#[test]
fn test_codec() {
    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::Missing
    }));
    assert!(codec(Parcel {
        ka_rq: Some(10),
        ka_ok: None,
        body: ParcelBody::Missing
    }));
    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: Some(20),
        body: ParcelBody::Missing
    }));
    assert!(codec(Parcel {
        ka_rq: Some(20),
        ka_ok: Some(20),
        body: ParcelBody::Missing
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: None,
            body: MsgDataBody::Missing,
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: Some(30),
            body: MsgDataBody::Missing,
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: Some(30),
            body: MsgDataBody::MsgSync(MsgSync {
                brd: 30,
                one: 40,
            }),
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: Some(30),
            body: MsgDataBody::MsgFinal(MsgFinal {
                brd: 30,
                one: 40,
            }),
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: Some(30),
            body: MsgDataBody::MsgBrd(MsgBrd {
                seq: 30,
                data: b"hello".to_vec(),
            }),
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgData(MsgData {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: Some(30),
            body: MsgDataBody::MsgOne(MsgOne {
                seq: 40,
                data: b"hello".to_vec(),
            }),
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::MsgAck(MsgAck {
            to: Sid::new("abc"),
            fr: Sid::new("def"),
            id: 30,
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::LcGossip(LcGossip {
            rows: HashMap::new(),
            cols: Vec::new(),
        }),
    }));

    assert!(codec(Parcel {
        ka_rq: None,
        ka_ok: None,
        body: ParcelBody::LcGossip(LcGossip {
            rows: {
                let mut rows = HashMap::new();
                rows.insert(
                    Sid::new("AAA"),
                    vec![Timespec::new(3, 4), Timespec::new(5, 6)],
                );
                rows.insert(
                    Sid::new("BBB"),
                    vec![Timespec::new(1, 2), Timespec::new(7, 8)],
                );
                rows
            },
            cols: vec![Sid::new("CCC"), Sid::new("DDD")],
        }),
    }));
}
