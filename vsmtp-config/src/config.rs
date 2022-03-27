/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::use_self)]
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
    #[serde(default)]
    pub server: ConfigServer,
    #[serde(default)]
    pub app: ConfigApp,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServer {
    // TODO: parse valid fqdn
    #[serde(default = "ConfigServer::hostname")]
    pub domain: String,
    #[serde(default = "ConfigServer::default_client_count_max")]
    pub client_count_max: i64,
    #[serde(default)]
    pub system: ConfigServerSystem,
    #[serde(default)]
    pub interfaces: ConfigServerInterfaces,
    #[serde(default)]
    pub logs: ConfigServerLogs,
    #[serde(default)]
    pub queues: ConfigServerQueues,
    pub tls: Option<ConfigServerTls>,
    #[serde(default)]
    pub smtp: ConfigServerSMTP,
    #[serde(default)]
    pub dns: ConfigServerDNS,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSystem {
    // TODO: should be users::
    #[serde(default = "ConfigServerSystem::default_user")]
    pub user: String,
    // TODO: should be users::
    #[serde(default = "ConfigServerSystem::default_group")]
    pub group: String,
    #[serde(default)]
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
    // TODO: parse valid fqdn
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
    #[serde(default = "ConfigServerSMTP::default_rcpt_count_max")]
    pub rcpt_count_max: usize,
    #[serde(default = "ConfigServerSMTP::default_disable_ehlo")]
    pub disable_ehlo: bool,
    // TODO: parse extension enum
    #[serde(default = "ConfigServerSMTP::default_required_extension")]
    pub required_extension: Vec<String>,
    #[serde(default)]
    pub error: ConfigServerSMTPError,
    #[serde(default)]
    pub timeout_client: ConfigServerSMTPTimeoutClient,
    #[serde(default)]
    pub codes: std::collections::BTreeMap<SMTPReplyCode, String>,
    // NOTE: extension settings here
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ConfigServerDNS {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "cloudflare")]
    CloudFlare,
    #[serde(rename = "custom")]
    Custom {
        config: trust_dns_resolver::config::ResolverConfig,
        options: trust_dns_resolver::config::ResolverOpts,
    },
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
    #[serde(default = "ConfigApp::default_dirpath")]
    pub dirpath: std::path::PathBuf,
    #[serde(default)]
    pub vsl: ConfigAppVSL,
    #[serde(default)]
    pub logs: ConfigAppLogs,
    #[serde(default)]
    pub services: std::collections::BTreeMap<String, Service>,
}
