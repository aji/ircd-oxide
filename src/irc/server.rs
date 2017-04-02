use std::io;
use std::str::FromStr;
use std::net::SocketAddr;

use futures::Future;
use futures::Sink;
use futures::Stream;
use futures::stream;
use futures::sync::mpsc;

use tokio_io::AsyncRead;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;
use tokio_core::net::TcpStream;

use irc::codec::IrcCodec;
use irc::client::Client;

pub fn listen(handle: Handle, addr: &SocketAddr) -> io::Result<()> {
    let listener = try!(TcpListener::bind(addr, &handle));

    let inner_handle = handle.clone();
    let conn_handler = listener.incoming().for_each(move |(conn, _)| {
        bind_client(inner_handle.clone(), conn)
    }).map_err(|e| {
        println!("error: listener shutting down: {}", e);
    });

    handle.spawn(conn_handler);

    Ok(())
}

pub fn bind_client(handle: Handle, conn: TcpStream) -> io::Result<()> {
    let (sink, stream) = conn.framed(IrcCodec).split();
    let (sender, receiver) = mpsc::unbounded();

    let client = Client::new(sender);

    let msg_handler = stream.fold(client, |client, msg| {
        println!("-> {:?}", msg);
        client.handle(msg)
    }).map_err(|e| {
        println!("error: client shutting down: {}", e);
    }).map(|ok| {
        println!("input stream ended");
    });

    let out_handler = receiver.fold(sink, |sink, msg| {
        sink.send(msg).map_err(|e| ())
    }).map(|ok| {
        println!("output stream ended");
    });

    handle.spawn(msg_handler);
    handle.spawn(out_handler);

    Ok(())
}
