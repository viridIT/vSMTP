/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
#![allow(clippy::use_self)] // false positive
#![allow(missing_docs)]

use serde_with::serde_as;
use vsmtp_common::{auth::Mechanism, re::log, CodeID, Reply};

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
    pub server: FieldServer,
    #[serde(default)]
    pub app: FieldApp,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServer {
    // TODO: parse valid fqdn
    #[serde(default = "FieldServer::hostname")]
    pub domain: String,
    #[serde(default = "FieldServer::default_client_count_max")]
    pub client_count_max: i64,
    #[serde(default)]
    pub system: FieldServerSystem,
    #[serde(default)]
    pub interfaces: FieldServerInterfaces,
    #[serde(default)]
    pub logs: FieldServerLogs,
    #[serde(default)]
    pub queues: FieldServerQueues,
    pub tls: Option<FieldServerTls>,
    #[serde(default)]
    pub smtp: FieldServerSMTP,
    #[serde(default)]
    pub dns: FieldServerDNS,
    #[serde(default)]
    pub r#virtual: std::collections::BTreeMap<String, FieldServerVirtual>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerSystem {
    #[serde(default = "FieldServerSystem::default_user")]
    #[serde(
        serialize_with = "crate::parser::syst_user::serialize",
        deserialize_with = "crate::parser::syst_user::deserialize"
    )]
    pub user: users::User,
    #[serde(default = "FieldServerSystem::default_group")]
    #[serde(
        serialize_with = "crate::parser::syst_group::serialize",
        deserialize_with = "crate::parser::syst_group::deserialize"
    )]
    pub group: users::Group,
    #[serde(default)]
    #[serde(
        serialize_with = "crate::parser::syst_group::opt_serialize",
        deserialize_with = "crate::parser::syst_group::opt_deserialize"
    )]
    pub group_local: Option<users::Group>,
    #[serde(default)]
    pub thread_pool: FieldServerSystemThreadPool,
}

impl PartialEq for FieldServerSystem {
    fn eq(&self, other: &Self) -> bool {
        self.user.uid() == other.user.uid()
            && self.group.gid() == other.group.gid()
            && self.thread_pool == other.thread_pool
    }
}

impl Eq for FieldServerSystem {}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerSystemThreadPool {
    pub receiver: usize,
    pub processing: usize,
    pub delivery: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerInterfaces {
    #[serde(default)]
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
    pub addr: Vec<std::net::SocketAddr>,
    #[serde(default)]
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
    pub addr_submission: Vec<std::net::SocketAddr>,
    #[serde(default)]
    #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
    pub addr_submissions: Vec<std::net::SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerLogs {
    #[serde(default = "FieldServerLogs::default_filepath")]
    pub filepath: std::path::PathBuf,
    #[serde(default = "FieldServerLogs::default_format")]
    pub format: String,
    #[serde(default = "FieldServerLogs::default_level")]
    pub level: std::collections::BTreeMap<String, log::LevelFilter>,
    #[serde(default = "FieldServerLogs::default_size_limit")]
    pub size_limit: u64,
    #[serde(default = "FieldServerLogs::default_archive_count")]
    pub archive_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldQueueWorking {
    #[serde(default = "FieldQueueWorking::default_channel_size")]
    pub channel_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldQueueDelivery {
    #[serde(default = "FieldQueueDelivery::default_channel_size")]
    pub channel_size: usize,
    #[serde(default = "FieldQueueDelivery::default_deferred_retry_max")]
    pub deferred_retry_max: usize,
    #[serde(with = "humantime_serde")]
    #[serde(default = "FieldQueueDelivery::default_deferred_retry_period")]
    pub deferred_retry_period: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerQueues {
    pub dirpath: std::path::PathBuf,
    #[serde(default)]
    pub working: FieldQueueWorking,
    #[serde(default)]
    pub delivery: FieldQueueDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerVirtual {
    pub tls: Option<FieldServerVirtualTls>,
    pub dns: Option<FieldServerDNS>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerVirtualTls {
    #[serde(
        serialize_with = "crate::parser::tls_protocol_version::serialize",
        deserialize_with = "crate::parser::tls_protocol_version::deserialize"
    )]
    pub protocol_version: Vec<rustls::ProtocolVersion>,
    pub certificate: TlsFile<rustls::Certificate>,
    pub private_key: TlsFile<rustls::PrivateKey>,
    #[serde(default = "FieldServerVirtualTls::default_sender_security_level")]
    pub sender_security_level: TlsSecurityLevel,
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
    /// DANE protocol using TLSA dns records to establish a secure connection with a distant server.
    Dane { port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct TlsFile<T> {
    #[serde(skip_serializing)]
    pub inner: T,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerTls {
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
        serialize_with = "crate::parser::tls_cipher_suite::serialize",
        deserialize_with = "crate::parser::tls_cipher_suite::deserialize",
        default = "FieldServerTls::default_cipher_suite"
    )]
    pub cipher_suite: Vec<rustls::CipherSuite>,
    pub certificate: TlsFile<rustls::Certificate>,
    pub private_key: TlsFile<rustls::PrivateKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerSMTPError {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerSMTPTimeoutClient {
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
pub struct FieldServerSMTPAuth {
    #[serde(default = "FieldServerSMTPAuth::default_must_be_authenticated")]
    pub must_be_authenticated: bool,
    #[serde(default = "FieldServerSMTPAuth::default_enable_dangerous_mechanism_in_clair")]
    pub enable_dangerous_mechanism_in_clair: bool,
    #[serde(default = "FieldServerSMTPAuth::default_mechanisms")]
    pub mechanisms: Vec<Mechanism>,
    #[serde(default = "FieldServerSMTPAuth::default_attempt_count_max")]
    pub attempt_count_max: i64,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldServerSMTP {
    #[serde(default = "FieldServerSMTP::default_rcpt_count_max")]
    pub rcpt_count_max: usize,
    #[serde(default = "FieldServerSMTP::default_disable_ehlo")]
    pub disable_ehlo: bool,
    // TODO: parse extension enum
    #[serde(default = "FieldServerSMTP::default_required_extension")]
    pub required_extension: Vec<String>,
    #[serde(default)]
    pub error: FieldServerSMTPError,
    #[serde(default)]
    pub timeout_client: FieldServerSMTPTimeoutClient,
    #[serde(default)]
    #[serde_as(as = "std::collections::BTreeMap<serde_with::DisplayFromStr, _>")]
    pub codes: std::collections::BTreeMap<CodeID, Reply>,
    // NOTE: extension settings here
    pub auth: Option<FieldServerSMTPAuth>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FieldServerDNS {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "google")]
    Google {
        #[serde(default)]
        options: ResolverOptsWrapper,
    },
    #[serde(rename = "cloudflare")]
    CloudFlare {
        #[serde(default)]
        options: ResolverOptsWrapper,
    },
    #[serde(rename = "custom")]
    Custom {
        config: trust_dns_resolver::config::ResolverConfig,
        #[serde(default)]
        options: ResolverOptsWrapper,
    },
}

// TODO: remove that and use serde_with
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ResolverOptsWrapper {
    /// Specify the timeout for a request. Defaults to 5 seconds
    #[serde(with = "humantime_serde")]
    #[serde(default = "ResolverOptsWrapper::default_timeout")]
    pub timeout: std::time::Duration,
    /// Number of retries after lookup failure before giving up. Defaults to 2
    #[serde(default = "ResolverOptsWrapper::default_attempts")]
    pub attempts: usize,
    /// Rotate through the resource records in the response (if there is more than one for a given name)
    #[serde(default = "ResolverOptsWrapper::default_rotate")]
    pub rotate: bool,
    /// Use DNSSec to validate the request
    #[serde(default = "ResolverOptsWrapper::default_dnssec")]
    pub dnssec: bool,
    /// The ip_strategy for the Resolver to use when lookup Ipv4 or Ipv6 addresses
    #[serde(default = "ResolverOptsWrapper::default_ip_strategy")]
    pub ip_strategy: trust_dns_resolver::config::LookupIpStrategy,
    /// Cache size is in number of records (some records can be large)
    #[serde(default = "ResolverOptsWrapper::default_cache_size")]
    pub cache_size: usize,
    /// Check /ect/hosts file before dns requery (only works for unix like OS)
    #[serde(default = "ResolverOptsWrapper::default_use_hosts_file")]
    pub use_hosts_file: bool,
    /// Number of concurrent requests per query
    ///
    /// Where more than one nameserver is configured, this configures the resolver to send queries
    /// to a number of servers in parallel. Defaults to 2; 0 or 1 will execute requests serially.
    #[serde(default = "ResolverOptsWrapper::default_num_concurrent_reqs")]
    pub num_concurrent_reqs: usize,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAppVSL {
    pub filepath: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAppLogs {
    #[serde(default = "FieldAppLogs::default_filepath")]
    pub filepath: std::path::PathBuf,
    #[serde(default = "FieldAppLogs::default_level")]
    pub level: log::LevelFilter,
    #[serde(default = "FieldAppLogs::default_format")]
    pub format: String,
    #[serde(default = "FieldAppLogs::default_size_limit")]
    pub size_limit: u64,
    #[serde(default = "FieldAppLogs::default_archive_count")]
    pub archive_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldApp {
    #[serde(default = "FieldApp::default_dirpath")]
    pub dirpath: std::path::PathBuf,
    #[serde(default)]
    pub vsl: FieldAppVSL,
    #[serde(default)]
    pub logs: FieldAppLogs,
}
