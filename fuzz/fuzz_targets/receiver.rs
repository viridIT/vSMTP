#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp::{
    config::server_config::ServerConfig,
    connection::{Connection, Kind},
    io_service::IoService,
    server::ServerVSMTP,
    test_helpers::{DefaultResolverTest, Mock},
};

fuzz_target!(|data: &[u8]| {
    let mut config = std::sync::Arc::new(
        ServerConfig::builder()
            .with_server_default_port("fuzz.server.com")
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/fuzz/", vsmtp::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build(),
    );

    config.smtp.error.soft_count = -1;

    let mut written_data = Vec::new();
    let mut mock = Mock::new(data.to_vec(), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::<Mock<'_>>::from_plain(
        Kind::Opportunistic,
        "0.0.0.0:0".parse().unwrap(),
        config,
        &mut io,
    )
    .unwrap();

    let _ = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => todo!(),
    }
    .block_on(ServerVSMTP::handle_connection::<
        DefaultResolverTest,
        Mock<'_>,
    >(
        &mut conn,
        std::sync::Arc::new(tokio::sync::Mutex::new(DefaultResolverTest)),
        None,
    ));
});
