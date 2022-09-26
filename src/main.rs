use netns_rs::NetNs;
use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        mpsc::{sync_channel, RecvTimeoutError, Sender, SyncSender},
        Arc, Mutex,
    },
    time::Duration,
};

struct UdpProxy {
    tx: SyncSender<Vec<u8>>,
}

fn main() {
    let public = NetNs::get("public").unwrap();

    let tcp_listener = public
        .run(|_| TcpListener::bind("10.10.10.10:80").unwrap())
        .unwrap();
    let udp_listener = public
        .run(|_| UdpSocket::bind("10.10.10.10:80").unwrap())
        .unwrap();
    let private = NetNs::get("private").unwrap();

    // Scope for threads, this is so we can just reference the private NS instead of opening it
    // every time.
    std::thread::scope(|s| {
        // Spawn a new thread for incomming UDP messages
        s.spawn(|| {
            // Keep track of messages and pipes.
            let proxy_cache = Arc::new(Mutex::new(HashMap::new()));
            loop {
                let mut buffer = vec![0; u16::MAX as usize];
                let (n, remote) = udp_listener.recv_from(&mut buffer).unwrap();
                // Only keep the actual data.
                buffer.truncate(n);
                // Lock separatly so the lock is not scoped.
                let mut cache_lock = proxy_cache.lock().unwrap();
                let tx_handle = cache_lock.entry(remote).or_insert_with(|| {
                    // Buffer at most 10 messages.
                    let (tx, rx) = sync_channel::<Vec<u8>>(10);
                    // Get a new owned handle to the cache
                    let proxy_cache = proxy_cache.clone();
                    // Bind on the target address as it is local, but let OS pick a free port.
                    let tx_s = private
                        .run(|_| UdpSocket::bind("172.20.0.2:0").unwrap())
                        .unwrap();
                    // Connect to the target port.
                    tx_s.connect("172.20.0.2:80").unwrap();
                    // Get a new handle to the socket to receive messages on.
                    let rx_s = tx_s.try_clone().unwrap();
                    // Get a new handle to the public socket to send from.
                    let udp_sender = udp_listener.try_clone().unwrap();
                    s.spawn(move || {
                        loop {
                            // If we don't get a value after 1 minute, assume the "connection" is gone.
                            let data = match rx.recv_timeout(Duration::from_secs(60)) {
                                Ok(d) => d,
                                Err(e) => {
                                    match e {
                                        RecvTimeoutError::Timeout => {
                                            // Disconnect the socket
                                            proxy_cache.lock().unwrap().remove(&remote);
                                            // Drain possible data frames to avoid a race condition
                                            loop {
                                                match rx.try_recv() {
                                                    Ok(data) => {
                                                        tx_s.send(&data).unwrap();
                                                    }
                                                    // Nothing to do.
                                                    // TODO: what if the error cause is "Empty"? In
                                                    // that case there is a rogue sender left
                                                    // somewhere.
                                                    Err(_) => {}
                                                }
                                            }
                                        }
                                        // We are done here
                                        _ => return,
                                    }
                                }
                            };
                            tx_s.send(&data).unwrap();
                        }
                    });
                    std::thread::spawn(move || {
                        let mut buffer = [0; u16::MAX as usize];
                        let n = rx_s.recv(&mut buffer).unwrap();
                        udp_sender.send_to(&buffer[..n], remote).unwrap();
                    });
                    UdpProxy { tx }
                });
                // Try send, and drop the data in the sending failed. This means a packet gets dropped
                // if the receiver can't keep up with the sender, which is fine as the alternative
                // means the receiver (which is out of our controll) could cause a huge memory buildup
                // if it can't keep up. Also note that we have a limited buffer per connection for
                // short bursts.
                // There are 2 possible errors. The first one indicates the channel is full, in which
                // case we simply drop the data as explained. The second one means there are no more
                // receivers. Due to the way the locking happens this _should_ be logically impossible,
                // as the receiver only exits while it holds the lock and drains the channel, after
                // removing itself from the cache while holding said lock. Therefore, for this thread
                // to aquire the lock it must either do so before the receiver drops, in which case the
                // receiver will still handle this as part of its clean up code, or after, in which
                // case a new receiver is inserted.
                _ = tx_handle.tx.try_send(buffer);
            }
        });
    });

    loop {
        let (mut con, remote) = tcp_listener.accept().unwrap();
        println!("Accepted new connection from {}", remote);

        let mut remote = private
            .run(|_| TcpStream::connect("172.20.0.2:80").unwrap())
            .unwrap();

        // Make sure the proxy connection has the same properties as the initial connection
        remote.set_ttl(con.ttl().unwrap()).unwrap();
        remote.set_nodelay(con.nodelay().unwrap()).unwrap();
        remote
            .set_read_timeout(con.read_timeout().unwrap())
            .unwrap();
        remote
            .set_write_timeout(con.write_timeout().unwrap())
            .unwrap();

        let mut listener_writer = con
            .try_clone()
            .expect("Can create a new handle to listener socket");
        let mut remote_writer = remote
            .try_clone()
            .expect("Can create a new handle to a proxy socket");

        std::thread::Builder::new()
            .name("frontend_proxy".to_string())
            .spawn(move || {
                println!("Start copy frontend -> backend");
                match io::copy(&mut con, &mut remote_writer) {
                    // Copy stopped, meaning we got an EOF from remote.
                    Ok(_) => {}
                    // Copy got an error, but we are not sure which side it came from
                    Err(_) => {
                        // Try to shut down remote in case the error is caused by the other side
                        // Don't care about error shutting down
                        let _ = con.shutdown(std::net::Shutdown::Both);
                    }
                };
                println!("Stop copy remote -> frontend");
                // Again don't care about errors here, this is best effort
                let _ = remote_writer.shutdown(std::net::Shutdown::Both);
            })
            .unwrap();

        std::thread::Builder::new()
            .name("backend_proxy".to_string())
            .spawn(move || {
                println!("Start copy remote -> frontend");
                match io::copy(&mut remote, &mut listener_writer) {
                    // Copy stopped, meaning we got an EOF from remote.
                    Ok(_) => {}
                    // Copy got an error, but we are not sure which side it came from
                    Err(_) => {
                        // Try to shut down remote in case the error is caused by the other side
                        // Don't care about error shutting down
                        let _ = remote.shutdown(std::net::Shutdown::Both);
                    }
                };
                println!("Stop copy remote -> frontend");
                // Again don't care about errors here, this is best effort
                let _ = listener_writer.shutdown(std::net::Shutdown::Both);
            })
            .unwrap();
    }
}
