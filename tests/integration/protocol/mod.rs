use vsmtp::config::server_config::ServerConfig;

pub mod clair;
pub mod rset;
pub mod utf8;

fn get_test_config() -> std::sync::Arc<ServerConfig> {
    let mut c: ServerConfig =
        toml::from_str(include_str!("clair.config.toml")).expect("cannot parse config from toml");
    c.prepare();
    std::sync::Arc::new(c)
}
