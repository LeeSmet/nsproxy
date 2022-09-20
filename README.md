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

To test while also getting some performance metrics, `iperf2` can be used (`iperf3`
can also be used and probably achieve better results, however the code is currently
only meant for testing and doesn't play nice with the concurrent connections created
by `iperf3` in its default configuration).

In a new terminal, start the server:

`sudo ip net ex private iperf -sp 80 -i 1`

Then, in a third terminal, start the client:

`sudo ip net ex public iperf -c 10.10.10.10 -p 80 -i 1`

If you want to transfer data in both directions, the `--full-duplex` flag can be
added to the client.
