# coap-fileserver ![License](https://img.shields.io/crates/l/coap-fileserver) [![coap-fileserver on crates.io](https://img.shields.io/crates/v/coap-fileserver)](https://crates.io/crates/coap-fileserver)

## coap-fileserver: A simple CoAP file server

This crate provides simple static file service over CoAP.

While currently minimal (it can serve files but crashes at directory listings, an has no
security setup), it aims to:

* Provide correct and performant read-only access to files.
* Apply best practices of CoAP and server application security.

### Usage

Run as

```console
$ cargo install --path .
$ coap-fileserver .
```

(At the time of writing, the *only* directory that can be served is the working directory;
this will change as arguments are being added).
