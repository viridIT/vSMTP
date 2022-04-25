use super::unsafe_auth_config;
use anyhow::Context;
use vsmtp_common::{
    auth::Mechanism,
    re::{anyhow, base64, rsasl, strum},
};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::re::tokio;
use vsmtp_server::Server;
use vsmtp_server::{auth, ConnectionKind, ProcessMessage};

async fn test_auth(
    server_config: std::sync::Arc<Config>,
    expected_response: &'static [&str],
    port: u32,
    mech: Mechanism,
    rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
    (username, password): (&'static str, &'static str),
) -> anyhow::Result<()> {
    println!("running with mechanism {mech:?}");

    let socket_server = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    let (working_sender, _working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let server = tokio::spawn(async move {
        let (client_stream, client_addr) = socket_server.accept().await.unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::new(
                &server_config,
                &Some(server_config.app.vsl.filepath.clone()),
            )
            .context("failed to initialize the engine")
            .unwrap(),
        ));

        Server::run_session(
            client_stream,
            client_addr,
            ConnectionKind::Opportunistic,
            server_config,
            None,
            Some(rsasl),
            rule_engine,
            working_sender,
            delivery_sender,
        )
        .await
        .unwrap();
    });

    let client = tokio::spawn(async move {
        let mut stream = vsmtp_server::AbstractIO::new(
            tokio::net::TcpStream::connect(format!("0.0.0.0:{port}"))
                .await
                .unwrap(),
        );

        let mut rsasl = rsasl::SASL::new_untyped().unwrap();
        let mut session = rsasl.client_start(mech.to_string().as_str()).unwrap();

        session.set_property(rsasl::Property::GSASL_AUTHID, username.as_bytes());
        session.set_property(rsasl::Property::GSASL_PASSWORD, password.as_bytes());

        let greetings = stream.next_line(None).await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut stream, b"EHLO client.com\r\n")
            .await
            .unwrap();

        let mut output = vec![greetings];
        loop {
            let line = stream.next_line(None).await.unwrap();
            output.push(line);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
            break;
        }

        tokio::io::AsyncWriteExt::write_all(&mut stream, format!("AUTH {}\r\n", mech).as_bytes())
            .await
            .unwrap();

        loop {
            let line = base64::decode(
                stream
                    .next_line(None)
                    .await
                    .unwrap()
                    .strip_prefix("334 ")
                    .unwrap(),
            )
            .unwrap();

            let res = session.step(&line).unwrap();
            let (buffer, done) = match res {
                rsasl::Step::Done(buffer) => (buffer, true),
                rsasl::Step::NeedsMore(buffer) => (buffer, false),
            };
            tokio::io::AsyncWriteExt::write_all(&mut stream, base64::encode(&**buffer).as_bytes())
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut stream, b"\r\n")
                .await
                .unwrap();

            if done {
                break;
            }
        }

        let line = stream.next_line(None).await.unwrap();
        output.push(line);

        pretty_assertions::assert_eq!(output, expected_response);
    });

    let (client, server) = tokio::join!(client, server);

    client.unwrap();
    server.unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn plain() {
    test_auth(
        std::sync::Arc::new(unsafe_auth_config()),
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-AUTH PLAIN LOGIN CRAM-MD5",
            "250-STARTTLS",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "235 2.7.0 Authentication succeeded",
        ],
        20015,
        Mechanism::Plain,
        {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            std::sync::Arc::new(tokio::sync::Mutex::new(rsasl))
        },
        ("hello", "world"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn login() {
    test_auth(
        std::sync::Arc::new(unsafe_auth_config()),
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-AUTH PLAIN LOGIN CRAM-MD5",
            "250-STARTTLS",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "235 2.7.0 Authentication succeeded",
        ],
        20016,
        Mechanism::Login,
        {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            std::sync::Arc::new(tokio::sync::Mutex::new(rsasl))
        },
        ("hello", "world"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn all_supported_by_rsasl() {
    let config = std::sync::Arc::new(unsafe_auth_config());

    let mut rsasl = rsasl::SASL::new_untyped().unwrap();
    rsasl.install_callback::<auth::Callback>();

    let rsasl = std::sync::Arc::new(tokio::sync::Mutex::new(rsasl));
    for mechanism in <Mechanism as strum::IntoEnumIterator>::iter() {
        test_auth(
            config.clone(),
            &[
                "220 testserver.com Service ready",
                "250-testserver.com",
                "250-AUTH PLAIN LOGIN CRAM-MD5",
                "250-STARTTLS",
                "250-8BITMIME",
                "250 SMTPUTF8",
                "235 2.7.0 Authentication succeeded",
            ],
            20017,
            mechanism,
            rsasl.clone(),
            ("hello", "world"),
        )
        .await
        .unwrap();
    }
}
