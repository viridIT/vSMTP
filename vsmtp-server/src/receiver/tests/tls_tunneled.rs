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
    processes::ProcessMessage,
    receiver::{connection::ConnectionKind, io_service::IoService},
    server::ServerVSMTP,
};
use vsmtp_common::re::anyhow;
use vsmtp_config::{
    get_rustls_config,
    re::{rustls, rustls_pemfile},
    Config, ConfigServerTlsSni, TlsSecurityLevel,
};
use vsmtp_rule_engine::rule_engine::RuleEngine;

use super::{get_tls_config, TEST_SERVER_CERT};

// using sockets on 2 thread to make the handshake concurrently
async fn test_tls_tunneled(
    server_name: &str,
    server_config: std::sync::Arc<Config>,
    smtp_input: &'static [&str],
    expected_output: &'static [&str],
    port: u32,
) -> anyhow::Result<()> {
    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
        RuleEngine::new(&Some(server_config.app.vsl.filepath.clone())).unwrap(),
    ));

    let server = tokio::spawn(async move {
        let tls_config = get_rustls_config(server_config.server.tls.as_ref().unwrap()).unwrap();

        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        ServerVSMTP::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Tunneled,
            server_config,
            Some(std::sync::Arc::new(tls_config)),
            None,
            rule_engine,
            working_sender,
            delivery_sender,
        )
        .await
        .unwrap();
    });

    let mut reader = std::io::BufReader::new(std::fs::File::open(&TEST_SERVER_CERT)?);

    let pem = rustls_pemfile::certs(&mut reader)
        .unwrap()
        .into_iter()
        .map(rustls::Certificate)
        .collect::<Vec<_>>();

    let mut root_store = rustls::RootCertStore::empty();
    for i in pem {
        root_store.add(&i).unwrap();
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

        let mut input = smtp_input.iter().copied();
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
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
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
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;
    config.server.tls.as_mut().unwrap().sni.push(
        ConfigServerTlsSni::from_path(
            "second.testserver.com",
            "./src/receiver/tests/certs/sni/second.certificate.crt",
            "./src/receiver/tests/certs/sni/second.privateKey.key",
        )
        .unwrap(),
    );

    test_tls_tunneled(
        "second.testserver.com",
        std::sync::Arc::new(config),
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
