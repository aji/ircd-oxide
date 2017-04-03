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

use irc::pluto::Pluto;

pub fn listen(handle: Handle, addr: &SocketAddr) -> io::Result<()> {
    let listener = try!(TcpListener::bind(addr, &handle));

    let pluto = Pluto::new();
    let inner_handle = handle.clone();
    let conn_handler = listener.incoming().for_each(move |(conn, _)| {
        bind_client(pluto.clone(), inner_handle.clone(), conn)
    }).map_err(|e| {
        println!("error: listener shutting down: {}", e);
    });

    handle.spawn(conn_handler);

    Ok(())
}

pub fn bind_client(pluto: Pluto, handle: Handle, conn: TcpStream) -> io::Result<()> {
    let (sink, stream) = conn.framed(IrcCodec).split();
    let (sender, receiver) = mpsc::unbounded();

    let client = Client::new(sender.clone());

    let inner_pluto = pluto.clone();
    let msg_handler = stream.fold(client, move |client, msg| {
        println!("-> {:?}", msg);
        client.handle(inner_pluto.clone(), msg)
    }).map_err(|e| {
        println!("error: client shutting down: {}", e);
    }).map(|_| {
        println!("input stream ended");
    });

    let out_handler = receiver.fold(sink, |sink, msg| {
        sink.send(msg).map_err(|_| ())
    }).map(|_| {
        println!("output stream ended");
    });

    let pluto_handler = pluto.observer().fold(sender, |sender, val| {
        sender.send(format!("value is now {}", val)).map_err(|_| ())
    }).map(|_| {
        println!("pluto stream ended");
    });

    handle.spawn(msg_handler);
    handle.spawn(out_handler);
    handle.spawn(pluto_handler);

    Ok(())
}
