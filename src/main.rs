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
