use crate::config::server_config::ServerConfig;

lazy_static::lazy_static! {
    pub static ref DEFAULT_CONFIG: ServerConfig = {
        let mut config = toml::from_str::<ServerConfig>(include_str!("../../config/vsmtp.default.toml"))
            .expect("Failed to load server config from toml");
            config.prepare_default();
            config
    };
}
