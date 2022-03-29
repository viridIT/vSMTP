use anyhow::Context;
use vsmtp_common::{
    auth::Mechanism,
    re::{base64, rsasl, strum},
};
use vsmtp_config::{Config, ConfigServerSMTPAuth};
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::{
    processes::ProcessMessage,
    receiver::{ConnectionKind, IoService},
    server::{SaslBackend, ServerVSMTP},
};

use super::{get_auth_config, TestAuth};

async fn test_auth(
    server_config: std::sync::Arc<Config>,
    expected_response: &'static [&str],
    port: u32,
    mech: Mechanism,
    rsasl: std::sync::Arc<tokio::sync::Mutex<SaslBackend>>,
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
            RuleEngine::new(&Some(server_config.app.vsl.filepath.clone()))
                .context("failed to initialize the engine")
                .unwrap(),
        ));

        ServerVSMTP::run_session(
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
        let mut client = std::net::TcpStream::connect(format!("0.0.0.0:{port}")).unwrap();
        let mut io = IoService::new(&mut client);

        let mut rsasl = rsasl::SASL::new_untyped().unwrap();
        let mut session = rsasl.client_start(mech.to_string().as_str()).unwrap();

        session.set_property(rsasl::Property::GSASL_AUTHID, username.as_bytes());
        session.set_property(rsasl::Property::GSASL_PASSWORD, password.as_bytes());

        let greetings = io.get_next_line_async().await.unwrap();
        std::io::Write::write_all(&mut io, b"EHLO client.com\r\n").unwrap();

        let mut output = vec![greetings];

        loop {
            let res = io.get_next_line_async().await.unwrap();
            output.push(res);
            if output.last().unwrap().chars().nth(3) == Some('-') {
                continue;
            }
            break;
        }

        std::io::Write::write_all(&mut io, format!("AUTH {}\r\n", mech).as_bytes()).unwrap();

        loop {
            let read = io.get_next_line_async().await.unwrap();
            let read = read.strip_prefix("334 ").unwrap();
            let read = base64::decode(read).unwrap();

            match session.step(&read) {
                Ok(rsasl::Step::Done(buffer)) => {
                    std::io::Write::write_all(&mut io, base64::encode(&**buffer).as_bytes())
                        .unwrap();
                    std::io::Write::write_all(&mut io, b"\r\n").unwrap();
                    break;
                }
                Ok(rsasl::Step::NeedsMore(buffer)) => {
                    std::io::Write::write_all(&mut io, base64::encode(&**buffer).as_bytes())
                        .unwrap();
                    std::io::Write::write_all(&mut io, b"\r\n").unwrap();
                }
                Err(e) => todo!("{e}"),
            }
        }

        let outcome = io.get_next_line_async().await.unwrap();
        output.push(outcome);

        assert_eq!(output, expected_response);
    });

    let (client, server) = tokio::join!(client, server);

    client.unwrap();
    server.unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn plain() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    test_auth(
        std::sync::Arc::new(config),
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250-SMTPUTF8",
            "250-AUTH PLAIN",
            "250 STARTTLS",
            "235 2.7.0 Authentication succeeded",
        ],
        20015,
        Mechanism::Plain,
        {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            std::sync::Arc::new(tokio::sync::Mutex::new(rsasl))
        },
        ("hello", "world"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn login() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    test_auth(
        std::sync::Arc::new(config),
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250-SMTPUTF8",
            "250-AUTH PLAIN",
            "250 STARTTLS",
            "235 2.7.0 Authentication succeeded",
        ],
        20016,
        Mechanism::Login,
        {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            std::sync::Arc::new(tokio::sync::Mutex::new(rsasl))
        },
        ("hello", "world"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn all_supported_by_rsasl() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });
    let config = std::sync::Arc::new(config);

    let mut rsasl = rsasl::SASL::new_untyped().unwrap();
    rsasl.install_callback::<TestAuth>();

    let rsasl = std::sync::Arc::new(tokio::sync::Mutex::new(rsasl));
    for mechanism in <Mechanism as strum::IntoEnumIterator>::iter() {
        test_auth(
            config.clone(),
            &[
                "220 testserver.com Service ready",
                "250-testserver.com",
                "250-8BITMIME",
                "250-SMTPUTF8",
                "250-AUTH PLAIN",
                "250 STARTTLS",
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
