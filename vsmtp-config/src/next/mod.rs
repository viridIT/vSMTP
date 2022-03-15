use crate::TlsSecurityLevel;

#[cfg(test)]
mod tests;

mod parser {
    pub mod semver;
    pub mod tls_certificate;
    pub mod tls_private_key;
    pub mod tls_protocol_version;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(
        serialize_with = "crate::next::parser::semver::serialize",
        deserialize_with = "crate::next::parser::semver::deserialize"
    )]
    pub version_requirement: semver::VersionReq,
    pub server: ConfigServer,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServer {
    // TODO:
    pub domain: String,
    pub client_count_max: u32,
    pub system: ConfigServerSystem,
    pub interfaces: ConfigServerInterfaces,
    pub logs: ConfigServerLogs,
    pub queues: ConfigServerQueues,
    pub tls: ConfigServerTls,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSystem {
    // TODO: should be users::
    pub user: String,
    // TODO: should be users::
    pub group: String,
    pub thread_pool: ConfigServerSystemThreadPool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSystemThreadPool {
    pub receiver: u32,
    pub processing: u32,
    pub delivery: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerInterfaces {
    #[serde(deserialize_with = "crate::parser::deserialize_socket_addr")]
    pub addr: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::parser::deserialize_socket_addr")]
    pub addr_submission: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::parser::deserialize_socket_addr")]
    pub addr_submissions: Vec<std::net::SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerLogs {
    pub filepath: std::path::PathBuf,
    pub format: String,
    pub level: std::collections::BTreeMap<String, log::LevelFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueWorking {
    channel_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueDelivery {
    pub channel_size: u32,
    pub deferred_retry_max: u32,
    pub deferred_retry_period: std::time::Duration,
    // dead_file_lifetime: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerQueues {
    pub dirpath: std::path::PathBuf,
    pub working: ConfigQueueWorking,
    pub delivery: ConfigQueueDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConfigServerTlsSni {
    // TODO:
    pub domain: String,
    #[serde(
        serialize_with = "crate::next::parser::tls_certificate::serialize",
        deserialize_with = "crate::next::parser::tls_certificate::deserialize"
    )]
    pub certificate: rustls::Certificate,
    #[serde(
        serialize_with = "crate::next::parser::tls_private_key::serialize",
        deserialize_with = "crate::next::parser::tls_private_key::deserialize"
    )]
    pub private_key: rustls::PrivateKey,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerTls {
    pub security_level: TlsSecurityLevel,
    pub preempt_cipherlist: bool,
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    #[serde(
        serialize_with = "crate::next::parser::tls_protocol_version::serialize",
        deserialize_with = "crate::next::parser::tls_protocol_version::deserialize"
    )]
    pub protocol_version: Vec<rustls::ProtocolVersion>,
    #[serde(
        serialize_with = "crate::next::parser::tls_certificate::serialize",
        deserialize_with = "crate::next::parser::tls_certificate::deserialize"
    )]
    pub certificate: rustls::Certificate,
    #[serde(
        serialize_with = "crate::next::parser::tls_private_key::serialize",
        deserialize_with = "crate::next::parser::tls_private_key::deserialize"
    )]
    pub private_key: rustls::PrivateKey,
    pub sni: Vec<ConfigServerTlsSni>,
}
