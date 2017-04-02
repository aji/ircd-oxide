use std::io;
use std::str;

use bytes::{BufMut, BytesMut};
use tokio_io::codec::Decoder;
use tokio_io::codec::Encoder;

use irc::message::Message;

pub struct IrcCodec;

impl Decoder for IrcCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, Self::Error> {
        loop {
            let n_loc = src.iter().position(|b| *b == b'\n');
            let r_loc = src.iter().position(|b| *b == b'\r');

            let (nl_start, nl_size) = match n_loc {
                None => return Ok(None), // no \n
                Some(i) => match r_loc {
                    Some(j) if j + 1 == i => (j, 2), // \r\n
                    _ => (i, 1), // \n
                },
            };

            let line = src.split_to(nl_start);
            src.split_to(nl_size);

            if line.len() != 0 {
                return match Message::parse(line) {
                    Ok(s) => Ok(Some(s)),
                    Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
                };
            }
        }
    }
}

impl Encoder for IrcCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put(item);
        dst.put(b'\r');
        dst.put(b'\n');

        Ok(())
    }
}
