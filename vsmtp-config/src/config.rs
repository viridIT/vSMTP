#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
use vsmtp_common::code::SMTPReplyCode;

use crate::parser::{tls_certificate, tls_private_key};

///
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(
        serialize_with = "crate::parser::semver::serialize",
        deserialize_with = "crate::parser::semver::deserialize"
    )]
    pub version_requirement: semver::VersionReq,
    pub server: ConfigServer,
    pub app: ConfigApp,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServer {
    // TODO:
    pub domain: String,
    pub client_count_max: i64,
    pub system: ConfigServerSystem,
    pub interfaces: ConfigServerInterfaces,
    pub logs: ConfigServerLogs,
    pub queues: ConfigServerQueues,
    pub tls: Option<ConfigServerTls>,
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
    pub receiver: usize,
    pub processing: usize,
    pub delivery: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerInterfaces {
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
    pub addr: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
    pub addr_submission: Vec<std::net::SocketAddr>,
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
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
    pub channel_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueDelivery {
    pub channel_size: usize,
    pub deferred_retry_max: usize,
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
        serialize_with = "crate::parser::tls_certificate::serialize",
        deserialize_with = "crate::parser::tls_certificate::deserialize"
    )]
    pub certificate: rustls::Certificate,
    #[serde(
        serialize_with = "crate::parser::tls_private_key::serialize",
        deserialize_with = "crate::parser::tls_private_key::deserialize"
    )]
    pub private_key: rustls::PrivateKey,
}

impl ConfigServerTlsSni {
    ///
    ///
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private key is not valid
    pub fn from_path(domain: &str, certificate: &str, private_key: &str) -> anyhow::Result<Self> {
        Ok(Self {
            domain: domain.to_string(),
            certificate: tls_certificate::from_string(certificate)?,
            private_key: tls_private_key::from_string(private_key)?,
        })
    }
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
        serialize_with = "crate::parser::tls_protocol_version::serialize",
        deserialize_with = "crate::parser::tls_protocol_version::deserialize"
    )]
    pub protocol_version: Vec<rustls::ProtocolVersion>,
    #[serde(
        serialize_with = "crate::parser::tls_certificate::serialize",
        deserialize_with = "crate::parser::tls_certificate::deserialize"
    )]
    pub certificate: rustls::Certificate,
    #[serde(
        serialize_with = "crate::parser::tls_private_key::serialize",
        deserialize_with = "crate::parser::tls_private_key::deserialize"
    )]
    pub private_key: rustls::PrivateKey,
    pub sni: Vec<ConfigServerTlsSni>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSMTPError {
    pub soft_count: i64,
    pub hard_count: i64,
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
    pub rcpt_count_max: usize,
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
