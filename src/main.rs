use netns_rs::NetNs;
use std::{
    io,
    net::{TcpListener, TcpStream},
};

fn main() {
    let public = NetNs::get("public").unwrap();

    let listener = public
        .run(|_| TcpListener::bind("10.10.10.10:80").unwrap())
        .unwrap();

    loop {
        let (con, remote) = listener.accept().unwrap();
        println!("Accepted new connection from {}", remote);

        let private = NetNs::get("private").unwrap();
        let remote = private
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

        std::thread::spawn(move || {
            std::thread::scope(|s| {
                let mut listener_reader: Box<dyn io::Read + Send> = Box::new(&con);
                let mut listener_writer: Box<dyn io::Write + Send> = Box::new(&con);
                let mut remote_reader: Box<dyn io::Read + Send> = Box::new(&remote);
                let mut remote_writer: Box<dyn io::Write + Send> = Box::new(&remote);
                let r = &remote;

                s.spawn(move || {
                    println!("Start copy frontend -> backend");
                    let c = io::copy(&mut listener_reader, &mut remote_writer).unwrap();
                    println!("Stop copy frontend -> backend ({} bytes copied)", c);
                    r.shutdown(std::net::Shutdown::Both).unwrap();
                });

                s.spawn(move || {
                    println!("Start copy remote -> frontend");
                    let c = io::copy(&mut remote_reader, &mut listener_writer).unwrap();
                    println!("Stop copy remote -> frontend ({} bytes copied)", c);
                });
            });
        });
    }
}
