use crate::{
    config::server_config::{ServerConfig, TlsSecurityLevel},
    processes::ProcessMessage,
    receiver::{
        connection::ConnectionKind,
        io_service::IoService,
        test_helpers::{test_receiver, DefaultResolverTest},
    },
    rules::rule_engine::RuleEngine,
    server::ServerVSMTP,
    tls_helpers::{get_cert_from_file, get_rustls_config},
};

fn get_regular_config() -> anyhow::Result<ServerConfig> {
    ServerConfig::builder()
        .with_rfc_port("test.server.com", None)
        .without_log()
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/delivery", crate::collection! {})
        .with_rules("./tmp/nothing")
        .with_default_reply_codes()
        .build()
}

const SERVER_CERT: &str = "./src/receiver/tests/certs/certificate.crt";

// using sockets on 2 thread to make the handshake concurrently
async fn test_starttls(
    server_name: &str,
    server_config: std::sync::Arc<ServerConfig>,
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
        let tls_config = get_rustls_config(server_config.smtps.as_ref().unwrap()).unwrap();

        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            anyhow::Context::context(
                RuleEngine::new(server_config.rules.dir.as_str()),
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
        let mut io = IoService::new(&mut client);

        let mut output = vec![];

        let mut input = clair_smtp_input.to_vec().into_iter();

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

        let mut input = secured_smtp_input.to_vec().into_iter();
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
        std::sync::Arc::new(
            ServerConfig::builder()
                .with_rfc_port("testserver.com", None)
                .without_log()
                .with_safe_default_smtps(
                    TlsSecurityLevel::May,
                    SERVER_CERT,
                    "./src/receiver/tests/certs/privateKey.key",
                    None,
                )
                .with_default_smtp()
                .with_delivery("./tmp/trash", crate::collection! {})
                .with_rules("./tmp/no_rules")
                .with_default_reply_codes()
                .build()
                .unwrap(),
        ),
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
        std::sync::Arc::new(get_regular_config().unwrap()),
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
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await;

    assert!(res.is_err());

    // (hard_error - soft_error) * error_delay
    assert!(before_test.elapsed().as_millis() >= 5 * 100);
}

#[tokio::test]
async fn test_receiver_8() -> anyhow::Result<()> {
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
        std::sync::Arc::new(
            ServerConfig::builder()
                .with_rfc_port("test.server.com", None)
                .without_log()
                .with_safe_default_smtps(TlsSecurityLevel::Encrypt, "dummy", "dummy", None)
                .with_default_smtp()
                .with_delivery("./tmp/delivery", crate::collection! {})
                .with_rules("./tmp/nothing")
                .with_default_reply_codes()
                .build()?
        )
    )
    .await
    .is_ok());

    Ok(())
}
