#![allow(clippy::module_name_repetitions)]
use vsmtp_common::code::SMTPReplyCode;

///
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(
        serialize_with = "crate::next::parser::semver::serialize",
        deserialize_with = "crate::next::parser::semver::deserialize"
    )]
    pub(crate) version_requirement: semver::VersionReq,
    pub(crate) server: ConfigServer,
    pub(crate) app: ConfigApp,
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
    pub smtp: ConfigServerSMTP,
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
    #[serde(deserialize_with = "crate::next::parser::socket_addr::deserialize")]
    pub addr: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::next::parser::socket_addr::deserialize")]
    pub addr_submission: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::next::parser::socket_addr::deserialize")]
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
    pub channel_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueDelivery {
    pub channel_size: u32,
    pub deferred_retry_max: u32,
    #[serde(with = "humantime_serde")]
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
#[serde(deny_unknown_fields)]
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

/// If a TLS configuration is provided, configure how the connection should be treated
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum TlsSecurityLevel {
    /// Connection may stay in plain text for theirs transaction
    ///
    /// Connection may upgrade at any moment with a TLS tunnel (using STARTTLS mechanism)
    May,
    /// Connection must be under a TLS tunnel (using STARTTLS mechanism or using port 465)
    Encrypt,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSMTPError {
    pub soft_count: u32,
    pub hard_count: u32,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSMTPTimeoutClient {
    #[serde(with = "humantime_serde")]
    pub connect: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub helo: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub mail_from: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub rcpt_to: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub data: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSMTP {
    pub rcpt_count_max: u32,
    pub disable_ehlo: bool,
    // TODO:
    pub required_extension: Vec<String>,
    pub error: ConfigServerSMTPError,
    pub timeout_client: ConfigServerSMTPTimeoutClient,
    pub codes: std::collections::BTreeMap<SMTPReplyCode, String>,
    // TODO: extension settings here
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigAppVSL {
    pub filepath: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigAppLogs {
    pub filepath: std::path::PathBuf,
    pub level: log::LevelFilter,
    pub format: String,
}

///
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Service {
    /// A service can be a program to run in a subprocess
    #[serde(rename = "shell")]
    UnixShell {
        #[serde(with = "humantime_serde")]
        /// a duration after which the subprocess will be forced-kill
        timeout: std::time::Duration,
        /// optional: a user to run the subprocess under
        #[serde(default)]
        user: Option<String>,
        /// optional: a group to run the subprocess under
        #[serde(default)]
        group: Option<String>,
        /// the command to execute in the subprocess
        command: String,
        /// optional: parameters directly given to the executed program (argc, argv)
        args: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigApp {
    pub dirpath: std::path::PathBuf,
    pub vsl: ConfigAppVSL,
    pub logs: ConfigAppLogs,
    pub services: std::collections::BTreeMap<String, Service>,
}
