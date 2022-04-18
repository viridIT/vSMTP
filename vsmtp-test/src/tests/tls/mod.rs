mod starttls;
mod tunneled;
mod tunneled_with_auth;

const TEST_SERVER_CERT: &str = "./src/tests/certs/certificate.crt";
const TEST_SERVER_KEY: &str = "./src/tests/certs/privateKey.key";

use vsmtp_common::re::anyhow;
use vsmtp_config::{
    get_rustls_config,
    re::{rustls, rustls_pemfile},
    Config,
};
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::auth;
use vsmtp_server::re::tokio;
use vsmtp_server::{ConnectionKind, IoService, ProcessMessage, Server};

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
        .without_services()
        .with_system_dns()
        .validate()
        .unwrap()
}

// using sockets on 2 thread to make the handshake concurrently
#[allow(clippy::too_many_lines)]
async fn test_starttls(
    server_name: &str,
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
            client_stream,
            client_addr,
            ConnectionKind::Opportunistic,
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
                anyhow::Context::context(
                    RuleEngine::new(
                        &server_config,
                        &Some(server_config.app.vsl.filepath.clone()),
                    ),
                    "failed to initialize the engine",
                )
                .unwrap(),
            )),
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

    let mut conn =
        rustls::ClientConnection::new(std::sync::Arc::new(config), server_name.try_into().unwrap())
            .unwrap();

    let client = tokio::spawn(async move {
        let mut client = std::net::TcpStream::connect(format!("0.0.0.0:{port}")).unwrap();
        let mut io = IoService::new(&mut client);

        let mut output = vec![];

        let mut input = clair_smtp_input.iter().copied();

        loop {
            let res = io.get_next_line_async().await.unwrap();
            output.push(res);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
            match input.next() {
                Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
                None => break,
            }
        }

        println!("{output:?}");

        let mut tls = rustls::Stream::new(&mut conn, &mut io);
        let mut io = IoService::new(&mut tls);
        println!("begin handshake");

        if let Err(e) = std::io::Write::flush(&mut io) {
            pretty_assertions::assert_eq!(expected_output, output);
            anyhow::bail!(e);
        }
        println!("end handshake");

        // TODO: assert on negotiated cipher ... ?

        let mut input = secured_smtp_input.iter().copied();

        std::io::Write::write_all(&mut io, input.next().unwrap().as_bytes()).unwrap();

        loop {
            let res = io.get_next_line_async().await.unwrap();
            output.push(res);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
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

// using sockets on 2 thread to make the handshake concurrently
async fn test_tls_tunneled(
    server_name: &str,
    server_config: std::sync::Arc<Config>,
    smtp_input: Vec<String>,
    expected_output: Vec<String>,
    port: u32,
    get_tls_config: fn(&Config) -> Option<std::sync::Arc<rustls::ServerConfig>>,
    get_auth_config: fn(&Config) -> Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
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
            get_tls_config(&server_config),
            get_auth_config(&server_config),
            std::sync::Arc::new(std::sync::RwLock::new(
                RuleEngine::new(
                    &server_config,
                    &Some(server_config.app.vsl.filepath.clone()),
                )
                .unwrap(),
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

        if let Err(e) = std::io::Write::flush(&mut io) {
            assert!(expected_output.is_empty());
            anyhow::bail!(e);
        }

        let mut output = vec![];

        // TODO: assert on negotiated cipher ... ?

        let mut input = smtp_input.iter().cloned();
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
