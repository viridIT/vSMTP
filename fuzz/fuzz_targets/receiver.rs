#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_config::server_config::ServerConfig;
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::{
    processes::ProcessMessage,
    receiver::{handle_connection, test_helpers::Mock, Connection, ConnectionKind, IoService},
};

fuzz_target!(|data: &[u8]| {
    let mut config = ServerConfig::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_rfc_port("fuzz.server.com", "root", "root", None)
        .without_log()
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/fuzz/")
        .with_empty_rules()
        .with_default_reply_codes()
        .build()
        .unwrap();
    config.smtp.error.soft_count = -1;

    let config = std::sync::Arc::new(config);

    let mut written_data = Vec::new();
    let mut mock = Mock::new(std::io::Cursor::new(data.to_vec()), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::from_plain(
        ConnectionKind::Opportunistic,
        "0.0.0.0:0".parse().unwrap(),
        config,
        &mut io,
    );

    let (working_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);
    let (delivery_sender, _) = tokio::sync::mpsc::channel::<ProcessMessage>(1);

    let re = std::sync::Arc::new(std::sync::RwLock::new(
        RuleEngine::new(&None).expect("failed to build rule engine"),
    ));

    let _ = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => todo!(),
    }
    .block_on(handle_connection(
        &mut conn,
        None,
        re,
        std::sync::Arc::new(working_sender),
        std::sync::Arc::new(delivery_sender),
    ));
});
