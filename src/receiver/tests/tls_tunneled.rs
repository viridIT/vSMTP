use std::io::Write;

use crate::{
    config::server_config::{ServerConfig, TlsSecurityLevel},
    processes::ProcessMessage,
    receiver::{
        connection::{Connection, ConnectionKind},
        handle_connection_secured,
        io_service::IoService,
        test_helpers::Mock,
    },
    server::ServerVSMTP,
    tls::get_rustls_config,
};

#[tokio::test]
async fn simple() -> anyhow::Result<()> {
    let server_config = std::sync::Arc::new(
        ServerConfig::builder()
            .with_server(
                "testserver.com",
                "0.0.0.0:10025".parse().expect("valid address"),
                "0.0.0.0:10587".parse().expect("valid address"),
                "0.0.0.0:10465".parse().expect("valid address"),
            )
            .without_log()
            .with_safe_default_smtps(
                TlsSecurityLevel::Encrypt,
                "./config/certs/certificate.crt",
                "./config/certs/privateKey.key",
                crate::collection! {},
            )
            .with_default_smtp()
            .with_delivery("./tmp/trash", crate::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build(),
    );
    let tls_config = server_config
        .tls
        .as_ref()
        .map(|t| get_rustls_config(&server_config.server.domain, t));

    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let client_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let mut conn = rustls::ClientConnection::new(
        std::sync::Arc::new(client_config),
        "testserver.com".try_into().unwrap(),
    )
    .unwrap();

    let mut written_data = vec![];
    let mut mock = Mock::new(std::io::Cursor::new(vec![]), &mut written_data);
    let mut tls = rustls::Stream::new(&mut conn, &mut mock);
    // tls.write_all(concat!("NOOP\r\n").as_bytes()).unwrap();

    let mut io = IoService::new(&mut tls);
    let mut conn = Connection::from_plain(
        ConnectionKind::Opportunistic,
        "0.0.0.0:10000".parse().unwrap(),
        server_config,
        &mut io,
    )?;

    let (working_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);
    let (delivery_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);

    handle_connection_secured(
        &mut conn,
        std::sync::Arc::new(working_sender),
        std::sync::Arc::new(delivery_sender),
        tls_config,
    )
    .await
    .unwrap();

    Ok(())
}

/*
#[tokio::test]
async fn simple() -> anyhow::Result<()> {
    let mut server = ServerVSMTP::new(std::sync::Arc::new(
        ServerConfig::builder()
            .with_server(
                "testserver.com",
                "0.0.0.0:10025".parse().expect("valid address"),
                "0.0.0.0:10587".parse().expect("valid address"),
                "0.0.0.0:10465".parse().expect("valid address"),
            )
            .without_log()
            .with_safe_default_smtps(
                TlsSecurityLevel::May,
                "./config/certs/certificate.crt",
                "./config/certs/privateKey.key",
                crate::collection! {},
            )
            .with_default_smtp()
            .with_delivery("./tmp/trash", crate::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build(),
    ))
    .await
    .unwrap();

    let listen = tokio::task::spawn(tokio::time::timeout(
        std::time::Duration::from_secs(1),
        async move { server.listen_and_serve().await },
    ));
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let client_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let mut client_conn = rustls::ClientConnection::new(
        std::sync::Arc::new(client_config),
        "testserver.com".try_into().unwrap(),
    )
    .unwrap();
    let mut sock = std::net::TcpStream::connect("127.0.0.1:10465").unwrap();

    let mut tls = rustls::Stream::new(&mut client_conn, &mut sock);
    std::io::Write::write_all(&mut tls, concat!("QUIT\r\n").as_bytes()).unwrap();

    let _ = listen.await.unwrap();
    Ok(())
}
*/
/*
#[tokio::test]
async fn simple() -> anyhow::Result<()> {
    let mut cursor_input = std::io::Cursor::new(
        concat!("HELO tunneled.client.com\r\n", "QUIT\r\n")
            .as_bytes()
            .to_vec(),
    );

    println!("{}", client_conn.is_handshaking());

    let mut stream = rustls::Stream::new(&mut client_conn, &mut cursor_input);
    std::io::Write::flush(&mut stream).unwrap();

    let mut written_data = Vec::new();
    let mut mock = Mock::new(stream, &mut written_data);

    let mut io = IoService::new(&mut mock);

    let server_config = get_rustls_config("testserver.com", full_config.tls.as_ref().unwrap());

    let mut conn = Connection::from_plain(
        ConnectionKind::Tunneled,
        "0.0.0.0:10578".parse().unwrap(),
        full_config,
        &mut io,
    )?;

    let (working_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    handle_connection_secured(
        &mut conn,
        std::sync::Arc::new(working_sender),
        std::sync::Arc::new(delivery_sender),
        Some(server_config),
    )
    .await
    .unwrap();

    Ok(())
}
*/
