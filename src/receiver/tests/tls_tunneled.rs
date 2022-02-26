/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use crate::{
    config::server_config::{ServerConfig, SniKey, TlsSecurityLevel},
    processes::ProcessMessage,
    receiver::{connection::ConnectionKind, io_service::IoService},
    rules::rule_engine::RuleEngine,
    server::ServerVSMTP,
    tls_helpers::{get_cert_from_file, get_rustls_config},
};

const SERVER_CERT: &str = "./src/receiver/tests/certs/certificate.crt";

// using sockets on 2 thread to make the handshake concurrently
async fn test_tls_tunneled(
    server_name: &str,
    server_config: std::sync::Arc<ServerConfig>,
    smtp_input: &'static [&str],
    expected_output: &'static [&str],
    port: u32,
) -> anyhow::Result<()> {
    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(
        server_config.rules.dir.clone(),
    )?));

    let server = tokio::spawn(async move {
        let tls_config = get_rustls_config(server_config.smtps.as_ref().unwrap()).unwrap();

        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        ServerVSMTP::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Tunneled,
            server_config,
            Some(std::sync::Arc::new(tls_config)),
            rule_engine,
            std::sync::Arc::new(working_sender),
            std::sync::Arc::new(delivery_sender),
        )
        .await
        .unwrap()
    });

    let mut root_store = rustls::RootCertStore::empty();
    for i in &get_cert_from_file(&std::path::PathBuf::from(SERVER_CERT)).unwrap() {
        root_store.add(i).unwrap();
    }

    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let mut conn =
        rustls::ClientConnection::new(std::sync::Arc::new(config), server_name.try_into().unwrap())
            .unwrap();

    let client = tokio::spawn(async move {
        let mut client = std::net::TcpStream::connect(format!("0.0.0.0:{port}")).unwrap();
        let mut tls = rustls::Stream::new(&mut conn, &mut client);
        let mut io = IoService::new(&mut tls);
        std::io::Write::flush(&mut io).unwrap();

        // TODO: assert on negotiated cipher ... ?

        let mut output = vec![];

        let mut input = smtp_input.to_vec().into_iter();
        loop {
            match io.get_next_line_async().await {
                Ok(res) => {
                    output.push(res);
                    match input.next() {
                        Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
                        None => break,
                    }
                }
                Err(e) => println!("{:?}", e),
            }
        }

        assert_eq!(output, expected_output);
    });

    let (client, server) = tokio::join!(client, server);

    client.unwrap();
    server.unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() -> anyhow::Result<()> {
    test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(
            ServerConfig::builder()
                .with_rfc_port("testserver.com", "foo", "foo", None)
                .without_log()
                .with_safe_default_smtps(
                    TlsSecurityLevel::Encrypt,
                    SERVER_CERT,
                    "./src/receiver/tests/certs/privateKey.key",
                    None,
                )
                .with_default_smtp()
                .with_delivery("./tmp/trash", crate::collection! {})
                .with_rules("./tmp/no_rules", vec![])
                .with_default_reply_codes()
                .build()
                .unwrap(),
        ),
        &[
            "NOOP\r\n",
            "HELO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<bar@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n",
        ],
        &[
            "220 testserver.com Service ready",
            "250 Ok",
            "250 Ok",
            "250 Ok",
            "250 Ok",
            "354 Start mail input; end with <CRLF>.<CRLF>",
            "250 Ok",
            "221 Service closing transmission channel",
        ],
        20466,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn sni() -> anyhow::Result<()> {
    test_tls_tunneled(
        "second.testserver.com",
        std::sync::Arc::new(
            ServerConfig::builder()
                .with_rfc_port("testserver.com", "foo", "foo", None)
                .without_log()
                .with_safe_default_smtps(
                    TlsSecurityLevel::Encrypt,
                    SERVER_CERT,
                    "./src/receiver/tests/certs/privateKey.key",
                    Some(vec![SniKey {
                        domain: "second.testserver.com".to_string(),
                        fullchain: "./src/receiver/tests/certs/sni/second.certificate.crt".into(),
                        private_key: "./src/receiver/tests/certs/sni/second.privateKey.key".into(),
                        protocol_version: None,
                    }]),
                )
                .with_default_smtp()
                .with_delivery("./tmp/trash", crate::collection! {})
                .with_rules("./tmp/no_rules", vec![])
                .with_default_reply_codes()
                .build()
                .unwrap(),
        ),
        &["NOOP\r\n", "QUIT\r\n"],
        &[
            "220 testserver.com Service ready",
            "250 Ok",
            "221 Service closing transmission channel",
        ],
        20467,
    )
    .await
}
