# NSProxy

A small research project to check a tcp proxy accross 2 local namespaces. A tcp
listener is opened in the first namespace. Once it receives a connection, a new
outbound connection is made from the second namespace. Once this second connection
is established, data is simply copied between the 2.
