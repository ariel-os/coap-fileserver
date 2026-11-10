//! coap-fileserver: A simple CoAP file server
//! ==========================================
//!
//! This crate provides simple static file service over CoAP.
//!
//! While currently minimal (it can serve files but crashes at directory listings, an has no
//! security setup), it aims to:
//!
//! * Provide correct and performant read-only access to files.
//! * Apply best practices of CoAP and server application security.
//!
//! Usage
//! -----
//!
//! Run as
//!
//! ```console
//! $ cargo install --path .
//! $ coap-fileserver .
//! ```
//!
//! (At the time of writing, the *only* directory that can be served is the working directory;
//! this will change as arguments are being added).

mod fileserver;

#[tokio::main]
async fn main() {
    let mut stack = std_embedded_nal_async::Stack::default();

    run_fileserver(&mut stack).await;
}

async fn run_fileserver(stack: &mut impl embedded_nal_async::UdpStack) {
    let mut sock = stack
        .bind_multiple(core::net::SocketAddr::new("::".parse().unwrap(), 5683))
        .await
        .expect("Can't create a socket");

    let mut handler = fileserver_handler();

    let coap = embedded_nal_coap::CoAPShared::<3>::new();
    let (_client, server) = coap.split();

    server
        .run(&mut sock, &mut handler, &mut rand::rng())
        .await
        .expect("UDP error")
}

fn fileserver_handler() -> impl coap_handler::Handler {
    fileserver::FileServer::new(cap_std::fs::Dir::from_std_file(
        std::fs::File::open(".").unwrap(),
    ))
}
