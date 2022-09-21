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
    let private = NetNs::get("private").unwrap();

    loop {
        let (mut con, remote) = listener.accept().unwrap();
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
