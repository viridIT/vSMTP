/**
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
**/
use serde_with::{serde_as, DisplayFromStr};

use crate::smtp::{code::SMTPReplyCode, state::StateSMTP};

use super::service::Service;

/// vSMTP's system server information
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerServerConfig {
    /// Domain sent to the client in the 'Greetings'
    pub domain: String,
    /// Machine's user to run the daemon
    pub vsmtp_user: String,
    /// Machine's group to run the daemon
    pub vsmtp_group: String,
    /// TCP/IP address of the rfc5321#section-4.5.4.2
    #[serde(default = "InnerServerConfig::default_addr")]
    pub addr: std::net::SocketAddr,
    /// TCP/IP address of the rfc6409
    #[serde(default = "InnerServerConfig::default_addr_submission")]
    pub addr_submission: std::net::SocketAddr,
    /// TCP/IP address of the rfc8314
    #[serde(default = "InnerServerConfig::default_addr_submissions")]
    pub addr_submissions: std::net::SocketAddr,
    /// The number of available worker thread in the runtime
    /// (default is the number of cores available to the system)
    #[serde(default = "num_cpus::get")]
    pub thread_count: usize,
}

/// vSMTP's system logs information
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerLogConfig {
    /// path of the vSMTP's system output
    #[serde(default = "InnerLogConfig::default_file")]
    pub file: std::path::PathBuf,
    // TODO: improve that
    /// Control the level of logging in the different area of the program
    ///
    /// keys are: [receiver, resolver, rules, deliver]
    #[serde(default)]
    // #[serde(serialize_with = "crate::config::serializer::ordered_map")]
    pub level: std::collections::BTreeMap<String, log::LevelFilter>,
}

/// vSMTP's application logs information
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerUserLogConfig {
    /// path of the vSMTP's application output
    pub file: std::path::PathBuf,
    /// level of the logs
    pub level: log::LevelFilter,
    // TODO: should not be optional
    /// format of the output of the logger following the log4rs [pattern]
    ///
    /// [pattern]: https://docs.rs/log4rs/latest/log4rs/encode/pattern/index.html
    #[serde(default)]
    pub format: Option<String>,
}

/// vSMTP's TLS security level
///
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

/// vSMTP's TLS Server Name Identification (SNI) parameters
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SniKey {
    /// name of the domain
    pub domain: String,
    /// path of the private key
    pub private_key: std::path::PathBuf,
    /// path of the certificate
    pub fullchain: std::path::PathBuf,
    /// optional requirement of the SSL/TLS protocol version for the domain
    pub protocol_version: Option<ProtocolVersionRequirement>,
}

/// Using a wrapping struct for serialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion(pub rustls::ProtocolVersion);

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProtocolVersionRequirement(pub Vec<ProtocolVersion>);

/// vSMTP's TLS configuration
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSmtpsConfig {
    /// see [TlsSecurityLevel]
    pub security_level: TlsSecurityLevel,
    /// requirement of the SSL/TLS protocol version
    pub protocol_version: ProtocolVersionRequirement,
    #[doc(hidden)]
    pub capath: std::path::PathBuf,
    /// Ignore the client's ciphersuite order. Instead, choose the top ciphersuite in the server list which is supported by the client.
    pub preempt_cipherlist: bool,
    /// path of the certificate
    pub fullchain: std::path::PathBuf,
    /// path of the private key
    pub private_key: std::path::PathBuf,
    /// maximum duration for the TLS handshake, produce a timeout too long
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    /// optional array of [SniKey]
    pub sni_maps: Option<Vec<SniKey>>,
}

/// specify how the client's error should be handled
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSMTPErrorConfig {
    /// after counting @soft_count, the server will delay the connection (by sleeping) for a @delay duration
    ///
    /// -1 to disable
    pub soft_count: i64,
    /// after counting @hard_count, the server will close the connection
    ///
    /// -1 to disable
    pub hard_count: i64,
    /// duration slept after @soft_count error
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
#[serde(deny_unknown_fields)]
pub struct DurationAlias {
    #[serde(with = "humantime_serde")]
    pub alias: std::time::Duration,
}

/// vSMTP's system protocol configuration
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSMTPConfig {
    /// should the server run without EHLO (and thus ESMTP) (default: false)
    #[serde(default)]
    pub disable_ehlo: bool,
    /// maximum delay for the next command for each step
    #[serde(default)]
    #[serde_as(as = "std::collections::BTreeMap<DisplayFromStr, _>")]
    pub timeout_client: std::collections::BTreeMap<StateSMTP, DurationAlias>,
    /// specify how the client's error should be handled
    pub error: InnerSMTPErrorConfig,
    /// maximum allowed recipients for a message
    #[serde(default = "InnerSMTPConfig::default_rcpt_count_max")]
    pub rcpt_count_max: usize,
    /// maximum number of client handled at the same time, any new connection will be closed
    ///
    /// -1 to disable
    #[serde(default = "InnerSMTPConfig::default_client_count_max")]
    pub client_count_max: i64,
}

/// vSMTP's application configuration
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerRulesConfig {
    /// entry point of the application
    #[serde(default)]
    pub main_filepath: Option<std::path::PathBuf>,
    /// application's logs configuration
    #[serde(default)]
    pub logs: InnerUserLogConfig,
    /// list of defined services to be run by the application
    #[serde(default)]
    pub services: Vec<Service>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueConfig {
    pub capacity: Option<usize>,
    pub retry_max: Option<usize>,
    #[serde(with = "humantime_serde", default)]
    pub cron_period: Option<std::time::Duration>,
}

/// vSMTP's system delivery configuration
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerDeliveryConfig {
    /// path of the spool directory where the processing queues write the files
    pub spool_dir: std::path::PathBuf,
    #[doc(hidden)]
    // #[serde(serialize_with = "crate::config::serializer::ordered_map")]
    pub queues: std::collections::BTreeMap<String, QueueConfig>,
}

/// the message sent to the client
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(transparent)]
pub struct Codes {
    /// key is the scenario, value is the message
    // #[serde(serialize_with = "crate::config::serializer::ordered_map")]
    pub codes: std::collections::BTreeMap<SMTPReplyCode, String>,
}

/// The server's configuration
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// the version required for vSMTP to parse this configuration
    #[serde(serialize_with = "crate::config::serializer::serialize_version_req")]
    pub version_requirement: semver::VersionReq,
    #[doc(hidden)]
    pub server: InnerServerConfig,
    #[doc(hidden)]
    pub log: InnerLogConfig,
    #[doc(hidden)]
    pub smtps: Option<InnerSmtpsConfig>,
    #[doc(hidden)]
    pub smtp: InnerSMTPConfig,
    #[doc(hidden)]
    pub delivery: InnerDeliveryConfig,
    #[doc(hidden)]
    pub rules: InnerRulesConfig,
    #[doc(hidden)]
    pub reply_codes: Codes,
}
