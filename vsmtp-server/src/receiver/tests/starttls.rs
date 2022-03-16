use vsmtp_config::{rustls_helper::get_rustls_config, tls_certificate, Config, TlsSecurityLevel};
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::{
    processes::ProcessMessage,
    receiver::{
        connection::ConnectionKind,
        io_service::IoService,
        test_helpers::{get_regular_config, test_receiver, DefaultResolverTest},
    },
    server::ServerVSMTP,
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
            rule_engine,
            working_sender,
            delivery_sender,
        )
        .await
        .unwrap();
    });

    let mut root_store = rustls::RootCertStore::empty();
    root_store
        .add(&tls_certificate::from_string(TEST_SERVER_CERT).unwrap())
        .unwrap();

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
            match io.get_next_line_async().await {
                Ok(res) => {
                    output.push(res);
                    if output.last().unwrap().chars().nth(3) == Some('-') {
                        continue;
                    }
                    match input.next() {
                        Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
                        None => break,
                    }
                }
                Err(e) => println!("{:?}", e),
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
        match input.next() {
            Some(line) => std::io::Write::write_all(&mut io, line.as_bytes()).unwrap(),
            None => panic!(),
        };

        loop {
            match io.get_next_line_async().await {
                Ok(res) => {
                    output.push(res);
                    if output.last().unwrap().chars().nth(3) == Some('-') {
                        continue;
                    }
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
            "250 STARTTLS",
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250 SMTPUTF8",
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
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["EHLO foobar\r\n", "STARTTLS\r\n", "QUIT\r\n"]
            .concat()
            .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250-test.server.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250 STARTTLS\r\n",
            "454 TLS not available due to temporary reason\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_9() {
    let before_test = std::time::Instant::now();
    let res = test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        [
            "RCPT TO:<bar@foo>\r\n",
            "MAIL FROM: <foo@bar>\r\n",
            "EHLO\r\n",
            "NOOP\r\n",
            "azeai\r\n",
            "STARTTLS\r\n",
            "MAIL FROM:<john@doe>\r\n",
            "EHLO\r\n",
            "EHLO\r\n",
            "HELP\r\n",
            "aieari\r\n",
            "not a valid smtp command\r\n",
        ]
        .concat()
        .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
            "503 Bad sequence of commands\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "250 Ok\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "503 Bad sequence of commands\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
    )
    .await;

    assert!(res.is_err());

    // (hard_error - soft_error) * error_delay
    assert!(before_test.elapsed().as_millis() >= 5 * 100);
}

#[tokio::test]
async fn test_receiver_8() -> anyhow::Result<()> {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["EHLO foobar\r\n", "MAIL FROM: <foo@bar>\r\n", "QUIT\r\n"]
            .concat()
            .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250-test.server.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250 STARTTLS\r\n",
            "530 Must issue a STARTTLS command first\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(config)
    )
    .await
    .is_ok());

    Ok(())
}
