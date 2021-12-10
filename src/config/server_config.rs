#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerServerConfig {
    pub addr: std::net::SocketAddr,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerLogConfig {
    pub file: String,
    pub level: std::collections::HashMap<String, log::LevelFilter>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TlsSecurityLevel {
    None,
    May,
    Encrypt,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct SniKey {
    pub domain: String,
    pub cert: String,
    pub chain: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerTlsConfig {
    pub security_level: TlsSecurityLevel,
    pub capath: Option<String>,
    pub preempt_cipherlist: bool,
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    pub sni_maps: Option<Vec<SniKey>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPErrorConfig {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPConfig {
    pub spool_dir: String,
    pub timeout_client: std::collections::HashMap<String, String>,
    pub error: InnerSMTPErrorConfig,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServerConfig {
    pub domain: String,
    pub version: String,
    pub server: InnerServerConfig,
    pub log: InnerLogConfig,
    pub tls: InnerTlsConfig,
    pub smtp: InnerSMTPConfig,
}
