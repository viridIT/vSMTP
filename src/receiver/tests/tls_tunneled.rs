use crate::{
    config::server_config::{ServerConfig, TlsSecurityLevel},
    processes::ProcessMessage,
    receiver::{connection::ConnectionKind, io_service::IoService},
    server::ServerVSMTP,
    tls::{get_cert_from_file, get_rustls_config},
};

// TODO: wrap this in a macro ..?

// TODO: add several test case with @input , @expected_output

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() -> anyhow::Result<()> {
    let socket_server = tokio::net::TcpListener::bind("0.0.0.0:20465").await?;

    let server = tokio::spawn(async move {
        let server_config = std::sync::Arc::new(
            ServerConfig::builder()
                .with_server_default_port("testserver.com")
                .without_log()
                .with_safe_default_smtps(
                    TlsSecurityLevel::Encrypt,
                    "./src/receiver/tests/certs/certificate.crt",
                    "./src/receiver/tests/certs/privateKey.key",
                    crate::collection! {},
                )
                .with_default_smtp()
                .with_delivery("./tmp/trash", crate::collection! {})
                .with_rules("./tmp/no_rules")
                .with_default_reply_codes()
                .build(),
        );

        let tls_config = get_rustls_config(
            &server_config.server.domain,
            server_config.smtps.as_ref().unwrap(),
        )
        .unwrap();

        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        let (working_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);
        let (delivery_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);

        ServerVSMTP::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Tunneled,
            server_config,
            Some(std::sync::Arc::new(tls_config)),
            std::sync::Arc::new(working_sender),
            std::sync::Arc::new(delivery_sender),
        )
        .await
        .unwrap()
    });

    let mut root_store = rustls::RootCertStore::empty();
    for i in &get_cert_from_file("./src/receiver/tests/certs/certificate.crt").unwrap() {
        root_store.add(i).unwrap();
    }

    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let mut conn = rustls::ClientConnection::new(
        std::sync::Arc::new(config),
        "testserver.com".try_into().unwrap(),
    )
    .unwrap();

    let client = tokio::spawn(async move {
        let mut client = std::net::TcpStream::connect("0.0.0.0:20465").unwrap();
        let mut tls = rustls::Stream::new(&mut conn, &mut client);
        let mut io = IoService::new(&mut tls);
        std::io::Write::flush(&mut io).unwrap();

        let expected_output = vec![
            "220 testserver.com Service ready",
            "250 Ok",
            "221 Service closing transmission channel",
        ];
        let mut input = vec!["NOOP\r\n", "QUIT\r\n"].into_iter();
        let mut output = vec![];

        loop {
            match io.get_next_line_async().await {
                Ok(res) => {
                    output.push(res);
                    match input.next() {
                        Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
                        None => break,
                    }
                }
                Err(_) => todo!(),
            }
        }

        assert_eq!(output, expected_output);
    });

    let (client, server) = tokio::join!(client, server);

    client.unwrap();
    server.unwrap();

    Ok(())
}
