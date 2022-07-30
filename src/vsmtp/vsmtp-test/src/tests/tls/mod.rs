/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
mod cipher_suite;
mod starttls;
mod tunneled;
mod tunneled_with_auth;

const TEST_SERVER_CERT: &str = "src/template/certs/certificate.crt";
const TEST_SERVER_KEY: &str = "src/template/certs/private_key.rsa.key";

use vsmtp_common::{
    re::{anyhow, tokio},
    ConnectionKind,
};
use vsmtp_config::{
    get_rustls_config,
    re::{rustls, rustls_pemfile},
    Config,
};
use vsmtp_rule_engine::RuleEngine;
use vsmtp_server::auth;
use vsmtp_server::Connection;
use vsmtp_server::{ProcessMessage, Server};

pub fn get_tls_config() -> Config {
    Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name("testserver.com")
        .with_user_group_and_default_system("root", "root")
        .unwrap()
        .with_ipv4_localhost()
        .with_default_logs_settings()
        .with_spool_dir_and_default_queues("./tmp/delivery")
        .with_safe_tls_config(TEST_SERVER_CERT, TEST_SERVER_KEY)
        .unwrap()
        .with_default_smtp_options()
        .with_default_smtp_error_handler()
        .with_default_smtp_codes()
        .without_auth()
        .with_default_app()
        .with_vsl("./src/tests/empty_main.vsl")
        .with_default_app_logs()
        .with_system_dns()
        .without_virtual_entries()
        .validate()
        .unwrap()
}

// using sockets on 2 thread to make the handshake concurrently
#[allow(clippy::too_many_lines)]
async fn test_starttls(
    server_name: &'static str,
    server_config: std::sync::Arc<Config>,
    clair_smtp_input: &'static [&str],
    secured_smtp_input: &'static [&str],
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
            Connection::new(
                ConnectionKind::Relay,
                client_addr,
                socket_server.local_addr().expect("retrieve local address"),
                server_config.clone(),
                client_stream,
            ),
            if with_valid_config {
                Some(std::sync::Arc::new(
                    get_rustls_config(
                        server_config.server.tls.as_ref().unwrap(),
                        &server_config.server.r#virtual,
                    )
                    .unwrap(),
                ))
            } else {
                None
            },
            None,
            std::sync::Arc::new(
                RuleEngine::new(&server_config, &server_config.app.vsl.filepath.clone()).unwrap(),
            ),
            std::sync::Arc::new(std::collections::HashMap::new()),
            working_sender,
            delivery_sender,
        )
        .await
    });

    let mut reader = std::io::BufReader::new(std::fs::File::open(&TEST_SERVER_CERT).unwrap());

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

    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));

    let client = tokio::spawn(async move {
        let mut stream = vsmtp_server::AbstractIO::new(
            tokio::net::TcpStream::connect(format!("0.0.0.0:{port}"))
                .await
                .unwrap(),
        );

        let mut output = vec![];
        let mut input = clair_smtp_input.iter().copied();

        loop {
            let line = stream.next_line(None).await.unwrap().unwrap();
            output.push(line);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
            match input.next() {
                Some(line) => {
                    tokio::io::AsyncWriteExt::write_all(&mut stream.inner, line.as_bytes())
                        .await
                        .unwrap();
                }
                None => break,
            }
        }

        println!("{output:?}");

        let mut stream = vsmtp_server::AbstractIO::new(
            connector
                .connect(
                    rustls::ServerName::try_from(server_name).unwrap(),
                    stream.inner,
                )
                .await?,
        );

        let mut input = secured_smtp_input.iter().copied();

        tokio::io::AsyncWriteExt::write_all(&mut stream.inner, input.next().unwrap().as_bytes())
            .await
            .unwrap();

        loop {
            let line = stream.next_line(None).await.unwrap().unwrap();
            output.push(line);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
            match input.next() {
                Some(line) => {
                    tokio::io::AsyncWriteExt::write_all(&mut stream.inner, line.as_bytes())
                        .await
                        .unwrap();
                }
                None => break,
            }
        }
        while let Ok(Some(last)) = stream.next_line(None).await {
            output.push(last);
        }

        pretty_assertions::assert_eq!(expected_output, output);

        anyhow::Ok(())
    });

    let (client, server) = tokio::join!(client, server);

    Ok((client.unwrap(), server.unwrap()))
}

// using sockets on 2 thread to make the handshake concurrently
#[allow(clippy::too_many_arguments)]
async fn test_tls_tunneled(
    server_name: &'static str,
    server_config: std::sync::Arc<Config>,
    smtp_input: Vec<String>,
    expected_output: Vec<String>,
    port: u32,
    get_tls_config: fn(&Config) -> Option<std::sync::Arc<rustls::ServerConfig>>,
    get_auth_config: fn(&Config) -> Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
    after_handshake: impl Fn(&tokio_rustls::client::TlsStream<tokio::net::TcpStream>) + 'static + Send,
) -> anyhow::Result<(anyhow::Result<()>, anyhow::Result<()>)> {
    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let server = tokio::spawn(async move {
        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        Server::run_session(
            Connection::new(
                ConnectionKind::Tunneled,
                client_addr,
                socket_server.local_addr().expect("retrieve local address"),
                server_config.clone(),
                client_stream,
            ),
            get_tls_config(&server_config),
            get_auth_config(&server_config),
            std::sync::Arc::new(
                RuleEngine::new(&server_config, &server_config.app.vsl.filepath.clone()).unwrap(),
            ),
            std::sync::Arc::new(std::collections::HashMap::new()),
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

    let client_config = std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );

    let connector = tokio_rustls::TlsConnector::from(client_config.clone());

    let client = tokio::spawn(async move {
        let stream = tokio::net::TcpStream::connect(format!("0.0.0.0:{port}"))
            .await
            .unwrap();
        let mut stream = vsmtp_server::AbstractIO::new(
            connector
                .connect(rustls::ServerName::try_from(server_name).unwrap(), stream)
                .await?,
        );

        let mut output = vec![];

        after_handshake(&stream.inner);

        let mut input = smtp_input.iter().cloned();
        loop {
            let line = stream.next_line(None).await.unwrap().unwrap();
            output.push(line);
            match input.next() {
                Some(line) => {
                    tokio::io::AsyncWriteExt::write_all(&mut stream.inner, line.as_bytes())
                        .await
                        .unwrap();
                }
                None => break,
            }
        }

        while let Ok(Some(last)) = stream.next_line(None).await {
            output.push(last);
        }

        pretty_assertions::assert_eq!(expected_output, output);

        anyhow::Ok(())
    });

    let (client, server) = tokio::join!(client, server);
    Ok((client.unwrap(), server.unwrap()))
}
