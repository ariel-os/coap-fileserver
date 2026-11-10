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
//! $ coap-fileserver --verbose .
//! ```
//!
//! Relevant specifications
//! -----------------------
//!
//! This runs a server as specified in [RFC7252](https://www.rfc-editor.org/info/rfc7252/),
//! including block-wise transfer specified in [RFC7959](https://www.rfc-editor.org/info/rfc7959/).
//!
//! The file system operations follow [the file system sketch in the kitchensink
//! draft](https://www.ietf.org/archive/id/draft-amsuess-core-coap-kitchensink-06.html#coap-filesystem).

mod fileserver;

use std::path::{Path, PathBuf};

#[derive(Debug, clap::Parser)]
#[command(version)]
struct Options {
    /// Path to serve as root path.
    pub path: PathBuf,

    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
}

#[tokio::main]
async fn main() {
    use clap::Parser;
    let options = Options::parse();

    let loglevel = options.verbose.log_level().unwrap_or(log::Level::Error);
    pretty_env_logger::formatted_builder()
        .filter(None, loglevel.to_level_filter())
        .try_init()
        .expect("There is only one logger setup");

    log::debug!("Verbosity set to {}", loglevel);

    let mut stack = std_embedded_nal_async::Stack::default();

    run_fileserver(&mut stack, &options.path).await;
}

async fn run_fileserver(stack: &mut impl embedded_nal_async::UdpStack, path: &Path) {
    let mut sock = stack
        .bind_multiple(core::net::SocketAddr::new("::".parse().unwrap(), 5683))
        .await
        .expect("Can't create a socket");

    let mut handler = fileserver_handler(path);

    let coap = embedded_nal_coap::CoAPShared::<3>::new();
    let (_client, server) = coap.split();

    server
        .run(&mut sock, &mut handler, &mut rand::rng())
        .await
        .expect("UDP error")
}

fn fileserver_handler(path: &Path) -> impl coap_handler::Handler {
    fileserver::FileServer::new(cap_std::fs_utf8::Dir::from_std_file(
        std::fs::File::open(path).unwrap(),
    ))
}
