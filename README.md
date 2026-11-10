# coap-fileserver ![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue) [![coap-fileserver on crates.io](https://img.shields.io/crates/v/coap-fileserver)](https://crates.io/crates/coap-fileserver) [![Source Code Repository](https://img.shields.io/badge/Code-On%20GitHub-blue?logo=GitHub)](https://github.com/ariel-os/coap-fileserver)

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
$ coap-fileserver --verbose .
```

### Relevant specifications

This runs a server as specified in [RFC7252][__link0],
including block-wise transfer specified in [RFC7959][__link1].

The file system operations follow [the file system sketch in the kitchensink
draft][__link2].


 [__link0]: https://www.rfc-editor.org/info/rfc7252/
 [__link1]: https://www.rfc-editor.org/info/rfc7959/
 [__link2]: https://www.ietf.org/archive/id/draft-amsuess-core-coap-kitchensink-06.html#coap-filesystem
