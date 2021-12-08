use crate::config::server_config::ServerConfig;

lazy_static::lazy_static! {
    pub static ref DEFAULT_CONFIG: ServerConfig = {
        toml::from_str(include_str!("../../config/vsmtp.default.toml")).unwrap()
    };
}
