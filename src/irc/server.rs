use std::cell::RefCell;
use std::io;
use std::net::SocketAddr;

use futures::Future;
use futures::Stream;

use tokio_core::net::TcpListener;
use tokio_core::reactor::Handle;

use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;
use tokio_io::codec::FramedRead;

use irc::codec::IrcCodec;
use irc::driver::Driver;
use irc::pending::Pending;
use irc::pluto::Pluto;
use irc::send::SendPool;
use irc::send::SendBinding;

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

struct ClientPool {
    handle: Handle,
    pluto: Pluto,
    out: SendPool,
}

impl ClientPool {
    fn new(handle: Handle, pluto: Pluto) -> ClientPool {
        let out = SendPool::new();
        let inner_out = out.clone();

        let observer = pluto.observer().for_each(move |ev| {
            info!("pluto update, val = {}, waiting 1ms...", ev);
            inner_out.send_all(format!("ATTN: value is now {}\r\n", ev));
            Ok(())
        });

        handle.spawn(observer);

        ClientPool {
            handle: handle,
            pluto: pluto,
            out: out,
        }
    }

    fn bind<R, W>(&mut self, recv: R, send: W)
        where R: 'static + AsyncRead,
              W: 'static + AsyncWrite
    {
        let out = self.out.clone();

        let recv_binding = FramedRead::new(recv, IrcCodec);
        let send_binding = SendBinding::new(send);

        let id = out.insert(send_binding.handle());
        let client = Pending::new(self.pluto.clone(), send_binding.handle());

        let mut soft_closer = send_binding.handle();
        let mut hard_closer = send_binding.handle();

        let driver = Driver::new(client, recv_binding);

        self.handle.spawn(driver.and_then(move |(active, recv)| {
            Driver::new(active, recv)
        }).and_then(move |(_, _)| {
            info!("receiver finished; closing writer (soft)");
            soft_closer.send(&b"Goodbye...\r\n"[..]);
            soft_closer.close_soft();
            Ok(())
        }).map_err(move |_| {
            info!("receiver errored; closing writer (hard)");
            hard_closer.close_hard();
        }));

        self.handle.spawn(send_binding.then(move |result| {
            out.remove(id);
            result
        }).map(|_| {
            info!("sender finished; nothing to do");
        }).map_err(|_| {
            info!("sender errored; nothing to do");
        }));
    }
}
