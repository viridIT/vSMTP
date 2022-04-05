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
use super::{get_tls_config, TEST_SERVER_CERT};
use vsmtp_common::re::anyhow;
use vsmtp_config::{
    get_rustls_config,
    re::{rustls, rustls_pemfile},
    Config, ConfigServerTlsSni, TlsSecurityLevel,
};
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::re::tokio;
use vsmtp_server::{ConnectionKind, IoService, ProcessMessage, Server};

// using sockets on 2 thread to make the handshake concurrently
async fn test_tls_tunneled(
    server_name: &str,
    server_config: std::sync::Arc<Config>,
    smtp_input: &'static [&str],
    expected_output: &'static [&str],
    port: u32,
    with_valid_config: bool,
) -> anyhow::Result<(anyhow::Result<()>, anyhow::Result<()>)> {
    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let server = tokio::spawn(async move {
        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        Server::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Tunneled,
            server_config.clone(),
            if with_valid_config {
                Some(std::sync::Arc::new(
                    get_rustls_config(server_config.server.tls.as_ref().unwrap()).unwrap(),
                ))
            } else {
                None
            },
            None,
            std::sync::Arc::new(std::sync::RwLock::new(
                RuleEngine::new(&Some(server_config.app.vsl.filepath.clone())).unwrap(),
            )),
            working_sender,
            delivery_sender,
        )
        .await
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

        let mut output = vec![];

        if let Err(e) = std::io::Write::flush(&mut io) {
            pretty_assertions::assert_eq!(expected_output, output);
            anyhow::bail!(e);
        }

        // TODO: assert on negotiated cipher ... ?

        let mut input = smtp_input.iter().copied();
        loop {
            let res = io.get_next_line_async().await.unwrap();
            output.push(res);
            match input.next() {
                Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
                None => break,
            }
        }

        if let Ok(Ok(last)) = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            io.get_next_line_async(),
        )
        .await
        {
            output.push(last);
        }

        pretty_assertions::assert_eq!(expected_output, output);

        anyhow::Ok(())
    });

    let (client, server) = tokio::join!(client, server);
    Ok((client.unwrap(), server.unwrap()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
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
        true,
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn starttls_under_tunnel() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
        &["NOOP\r\n", "STARTTLS\r\n"],
        &[
            "220 testserver.com Service ready",
            "250 Ok",
            "220 testserver.com Service ready",
            "554 5.5.1 Error: TLS already active",
        ],
        20467,
        true,
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn config_ill_formed() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
        &["NOOP\r\n"],
        &[],
        20461,
        false,
    )
    .await
    .unwrap();

    assert!(client.is_err());
    assert!(server.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn sni() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;
    config.server.tls.as_mut().unwrap().sni.push(
        ConfigServerTlsSni::from_path(
            "second.testserver.com",
            "./src/tests/certs/sni/second.certificate.crt",
            "./src/tests/certs/sni/second.privateKey.key",
        )
        .unwrap(),
    );

    let (client, server) = test_tls_tunneled(
        "second.testserver.com",
        std::sync::Arc::new(config),
        &["NOOP\r\n", "QUIT\r\n"],
        &[
            "220 testserver.com Service ready",
            "250 Ok",
            "221 Service closing transmission channel",
        ],
        20469,
        true,
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}
