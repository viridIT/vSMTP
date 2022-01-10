#![no_main]
use libfuzzer_sys::fuzz_target;

use vsmtp::{
    config::server_config::ServerConfig,
    connection::Connection,
    mailprocessing::io_service::IoService,
    server::handle_client,
    test_helpers::{DefaultResolverTest, Mock},
};

fuzz_target!(|data: &[u8]| {
    let mut config: ServerConfig =
        toml::from_str(include_str!("fuzz.config.toml")).expect("cannot parse config from toml");
    config.prepare();

    let mut written_data = Vec::new();
    let mut mock = Mock::new(data.to_vec(), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::<Mock<'_>>::from_plain(
        "0.0.0.0:0".parse().unwrap(),
        std::sync::Arc::new(config),
        &mut io,
    )
    .unwrap();

    let _ = handle_client::<DefaultResolverTest, Mock<'_>>(&mut conn, None);
});
