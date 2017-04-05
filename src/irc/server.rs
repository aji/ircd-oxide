use std::io;
use std::str::FromStr;
use std::net::SocketAddr;

use futures::Future;
use futures::Sink;
use futures::Stream;
use futures::stream;
use futures::unsync::mpsc;

use tokio_io::AsyncRead;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;
use tokio_core::net::TcpStream;

use irc::codec::IrcCodec;
use irc::client::Client;
use irc::client::ClientPool;

use irc::pluto::Pluto;

pub fn listen(handle: Handle, addr: &SocketAddr) -> io::Result<()> {
    let listener = try!(TcpListener::bind(addr, &handle));

    let mut client_pool = ClientPool::new(handle.clone(), Pluto::new());

    let conn_handler = listener.incoming().for_each(move |(conn, _)| {
        let (recv, send) = conn.split();
        client_pool.bind(recv, send);
        Ok(())
    }).map_err(|e| {
        println!("error: listener shutting down: {}", e);
    });

    handle.spawn(conn_handler);

    Ok(())
}
