# NSProxy

A small research project to check a tcp proxy accross 2 local namespaces. A tcp
listener is opened in the first namespace. Once it receives a connection, a new
outbound connection is made from the second namespace. Once this second connection
is established, data is simply copied between the 2.

## Building

Run `cargo build` in this directory, optionally adding the `--release` flag

## Testing

The code assumes the existence of a `public` and `private` namespace. The public
namespace must contain a link with ip `10.10.10.10`. Likewise, the private namespace
must contain a link with ip `172.20.0.2`. These IP addresses are just random, and
won't actually be used for routing. They merely exist to we can bind a tcp listener
and create a new tcp connection.

Setup of a testing environment can be done using [the included bash script](./setup_network.sh).

Once the test environment is set up, you can start the proxy (assuming a release
build): `sudo ./target/release/nsproxy`.

As test, we will proxy a small http file server. This shows that we can actually
proxy a real protocol.

In a new terminal, start the server:

`sudo ip net ex private python3 -m http.server 80`

Then, in a third terminal, fetch the data from the server:

`sudo ip net ex public curl http://10.10.10.10`

Since the focus of this project is to show proxy abilities between 2 different namespaces,
we created 2 isolated namespaces for testing, and hence we can't just point a browser
at it to have a more visual result. If this is desired, a veth pair can be created
between the default namespace and the public namespace:

```sh
sudo ip l add veth1 type veth peer veth2
sudo ip l set veth2 netns public
sudo ip r set 10.10.10.10/32 dev veth1
sudo ip -n public r add 192.168.0.0/16 dev veth2
sudo ip l set veth1 up
sudo ip -n public l set veth2 up
```

The file server can then be found by entering `http://10.10.10.10` in your brower.
Alternatively, you could also try and start your browser in the `public` network
namespace, though the above setup will mean you won't have any other connectivity.

**Note**: The above instructions asume that the IP in your default network namespace
is in the `192.168.0.0/16` subnet, and `10.10.10.10` is not actively used by a connection
on your network (as it _will_ break).

## Other options

Next to the presented work, it might also prove interesting to explore the following:

- Use the [tokio library] to see if the amount of spawned threads can be limited,
currently 2 are spawned for every client connection.
- Use [tokio with io-uring], as this might be more performant compared to the
regular [tokio library]. `io-uring` might also be used directly, and can then
possibly avoid copying data from kernel to userspace.
- Use ebpf sockmaps, [cloudflare already tried this], though it's been a couple
of years and performance will probably have improved since then.

[tokio library]: https://tokio.rs
[tokio with io-uring]: https://docs.rs/tokio-uring/latest/tokio_uring/
[cloudflare already tried this]: https://blog.cloudflare.com/sockmap-tcp-splicing-of-the-future/
