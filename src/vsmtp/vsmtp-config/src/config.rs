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
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::use_self)]

use crate::parser::{tls_certificate, tls_private_key};
use serde_with::serde_as;
use vsmtp_common::{
    auth::Mechanism,
    re::{anyhow, log},
    CodeID, Reply,
};

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
    #[serde(default)]
    pub r#virtual: std::collections::BTreeMap<String, ConfigServerVirtual>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerSystem {
    #[serde(default = "ConfigServerSystem::default_user")]
    #[serde(
        serialize_with = "crate::parser::syst_user::serialize",
        deserialize_with = "crate::parser::syst_user::deserialize"
    )]
    pub user: users::User,
    #[serde(default = "ConfigServerSystem::default_group")]
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
    pub thread_pool: ConfigServerSystemThreadPool,
}

impl PartialEq for ConfigServerSystem {
    fn eq(&self, other: &Self) -> bool {
        self.user.uid() == other.user.uid()
            && self.group.gid() == other.group.gid()
            && self.thread_pool == other.thread_pool
    }
}

impl Eq for ConfigServerSystem {}

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
pub struct ConfigServerLogs {
    #[serde(default = "ConfigServerLogs::default_filepath")]
    pub filepath: std::path::PathBuf,
    #[serde(default = "ConfigServerLogs::default_format")]
    pub format: String,
    #[serde(default = "ConfigServerLogs::default_level")]
    pub level: std::collections::BTreeMap<String, log::LevelFilter>,
    #[serde(default = "ConfigAppLogs::default_size_limit")]
    pub size_limit: u64,
    #[serde(default = "ConfigAppLogs::default_archive_count")]
    pub archive_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueWorking {
    #[serde(default = "ConfigQueueWorking::default_channel_size")]
    pub channel_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigQueueDelivery {
    #[serde(default = "ConfigQueueDelivery::default_channel_size")]
    pub channel_size: usize,
    #[serde(default = "ConfigQueueDelivery::default_deferred_retry_max")]
    pub deferred_retry_max: usize,
    #[serde(with = "humantime_serde")]
    #[serde(default = "ConfigQueueDelivery::default_deferred_retry_period")]
    pub deferred_retry_period: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerQueues {
    pub dirpath: std::path::PathBuf,
    #[serde(default)]
    pub working: ConfigQueueWorking,
    #[serde(default)]
    pub delivery: ConfigQueueDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerVirtual {
    pub tls: Option<ConfigServerVirtualTls>,
    pub dns: Option<ConfigServerDNS>,
}

impl ConfigServerVirtual {
    /// create a new virtual domain using the root domain parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tls: None,
            dns: None,
        }
    }

    /// create a new virtual domain with tls parameters.
    ///
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private key is not valid
    pub fn with_tls(certificate: &str, private_key: &str) -> anyhow::Result<Self> {
        Ok(Self {
            tls: Some(ConfigServerVirtualTls::from_path(certificate, private_key)?),
            dns: None,
        })
    }

    /// create a new virtual domain with a dns config.
    ///
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private key is not valid
    pub const fn with_dns(dns_config: ConfigServerDNS) -> anyhow::Result<Self> {
        Ok(Self {
            tls: None,
            dns: Some(dns_config),
        })
    }

    /// create a new virtual domain with a dns & tls parameters.
    ///
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private key is not valid
    pub fn with_tls_and_dns(
        certificate: &str,
        private_key: &str,
        dns_config: ConfigServerDNS,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            tls: Some(ConfigServerVirtualTls::from_path(certificate, private_key)?),
            dns: Some(dns_config),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigServerVirtualTls {
    #[serde(
        serialize_with = "crate::parser::tls_protocol_version::serialize",
        deserialize_with = "crate::parser::tls_protocol_version::deserialize"
    )]
    pub protocol_version: Vec<rustls::ProtocolVersion>,
    #[serde(
        // serialize_with = "crate::parser::tls_certificate::serialize",
        deserialize_with = "crate::parser::tls_certificate::deserialize"
    )]
    #[serde(skip_serializing)]
    pub certificate: rustls::Certificate,
    #[serde(
        // serialize_with = "crate::parser::tls_private_key::serialize",
        deserialize_with = "crate::parser::tls_private_key::deserialize"
    )]
    #[serde(skip_serializing)]
    pub private_key: rustls::PrivateKey,
    #[serde(default = "ConfigServerVirtualTls::default_sender_security_level")]
    pub sender_security_level: TlsSecurityLevel,
}

impl ConfigServerVirtualTls {
    /// create a virtual tls configuration from the certificate & private key paths.
    ///
    /// # Errors
    ///
    /// * certificate file not found.
    /// * private key file not found.
    pub fn from_path(certificate: &str, private_key: &str) -> anyhow::Result<Self> {
        Ok(Self {
            protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
            certificate: tls_certificate::from_string(certificate)?,
            private_key: tls_private_key::from_string(private_key)?,
            sender_security_level: ConfigServerVirtualTls::default_sender_security_level(),
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
    /// DANE protocol using TLSA dns records to establish a secure connection with a distant server.
    Dane { port: u16 },
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
        serialize_with = "crate::parser::tls_cipher_suite::serialize",
        deserialize_with = "crate::parser::tls_cipher_suite::deserialize",
        default = "ConfigServerTls::default_cipher_suite"
    )]
    pub cipher_suite: Vec<rustls::CipherSuite>,
    #[serde(
        // serialize_with = "crate::parser::tls_certificate::serialize",
        deserialize_with = "crate::parser::tls_certificate::deserialize"
    )]
    #[serde(skip_serializing)]
    pub certificate: rustls::Certificate,
    #[serde(
        // serialize_with = "crate::parser::tls_private_key::serialize",
        deserialize_with = "crate::parser::tls_private_key::deserialize"
    )]
    #[serde(skip_serializing)]
    pub private_key: rustls::PrivateKey,
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
pub struct ConfigServerSMTPAuth {
    #[serde(default = "ConfigServerSMTPAuth::default_must_be_authenticated")]
    pub must_be_authenticated: bool,
    #[serde(default = "ConfigServerSMTPAuth::default_enable_dangerous_mechanism_in_clair")]
    pub enable_dangerous_mechanism_in_clair: bool,
    #[serde(default = "ConfigServerSMTPAuth::default_mechanisms")]
    pub mechanisms: Vec<Mechanism>,
    #[serde(default = "ConfigServerSMTPAuth::default_attempt_count_max")]
    pub attempt_count_max: i64,
}

#[serde_as]
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
    #[serde_as(as = "std::collections::BTreeMap<serde_with::DisplayFromStr, _>")]
    pub codes: std::collections::BTreeMap<CodeID, Reply>,
    // NOTE: extension settings here
    pub auth: Option<ConfigServerSMTPAuth>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ConfigServerDNS {
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
pub struct ConfigAppVSL {
    pub filepath: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigAppLogs {
    #[serde(default = "ConfigAppLogs::default_filepath")]
    pub filepath: std::path::PathBuf,
    #[serde(default = "ConfigAppLogs::default_level")]
    pub level: log::LevelFilter,
    #[serde(default = "ConfigAppLogs::default_format")]
    pub format: String,
    #[serde(default = "ConfigAppLogs::default_size_limit")]
    pub size_limit: u64,
    #[serde(default = "ConfigAppLogs::default_archive_count")]
    pub archive_count: u32,
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
}
