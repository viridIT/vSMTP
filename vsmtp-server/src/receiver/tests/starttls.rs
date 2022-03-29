use vsmtp_config::{
    get_rustls_config,
    re::{rustls, rustls_pemfile},
    Config, TlsSecurityLevel,
};
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::{
    processes::ProcessMessage,
    receiver::{connection::ConnectionKind, io_service::IoService},
    server::ServerVSMTP,
    test_receiver,
};

use super::{get_tls_config, TEST_SERVER_CERT};

// using sockets on 2 thread to make the handshake concurrently
async fn test_starttls(
    server_name: &str,
    server_config: std::sync::Arc<Config>,
    clair_smtp_input: &'static [&str],
    secured_smtp_input: &'static [&str],
    expected_output: &'static [&str],
    port: u32,
) -> anyhow::Result<()> {
    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let server = tokio::spawn(async move {
        let tls_config = get_rustls_config(server_config.server.tls.as_ref().unwrap()).unwrap();

        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            anyhow::Context::context(
                RuleEngine::new(&Some(server_config.app.vsl.filepath.clone())),
                "failed to initialize the engine",
            )
            .unwrap(),
        ));

        ServerVSMTP::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Opportunistic,
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

        std::io::Write::flush(&mut io).unwrap();
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

        assert_eq!(output, expected_output);
    });

    let (client, server) = tokio::join!(client, server);

    client.unwrap();
    server.unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() -> anyhow::Result<()> {
    test_starttls(
        "testserver.com",
        std::sync::Arc::new(get_tls_config()),
        &["EHLO client.com\r\n", "STARTTLS\r\n"],
        &[
            "EHLO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<bar@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n",
        ],
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250-SMTPUTF8",
            "250-AUTH ",
            "250 STARTTLS",
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250-SMTPUTF8",
            "250 AUTH PLAIN LOGIN CRAM-MD5",
            "250 Ok",
            "250 Ok",
            "354 Start mail input; end with <CRLF>.<CRLF>",
            "250 Ok",
            "221 Service closing transmission channel",
        ],
        20027,
    )
    .await
}

#[tokio::test]
async fn test_receiver_7() {
    assert!(test_receiver! {
        [
            "EHLO foobar\r\n",
            "STARTTLS\r\n",
            "QUIT\r\n"
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH \r\n",
            "250 STARTTLS\r\n",
            "454 TLS not available due to temporary reason\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_8() -> anyhow::Result<()> {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    assert!(test_receiver! {
        with_config => config,
        ["EHLO foobar\r\n", "MAIL FROM: <foo@bar>\r\n", "QUIT\r\n"]
            .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH \r\n",
            "250 STARTTLS\r\n",
            "530 Must issue a STARTTLS command first\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());

    Ok(())
}
