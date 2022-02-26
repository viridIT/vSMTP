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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerServerConfig {
    pub domain: String,
    pub vsmtp_user: String,
    pub vsmtp_group: String,
    #[serde(default = "InnerServerConfig::default_addr")]
    pub addr: std::net::SocketAddr,
    #[serde(default = "InnerServerConfig::default_addr_submission")]
    pub addr_submission: std::net::SocketAddr,
    #[serde(default = "InnerServerConfig::default_addr_submissions")]
    pub addr_submissions: std::net::SocketAddr,
    #[serde(default = "num_cpus::get")]
    pub thread_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerLogConfig {
    #[serde(default = "InnerLogConfig::default_file")]
    pub file: std::path::PathBuf,
    #[serde(default)]
    pub level: std::collections::HashMap<String, log::LevelFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerUserLogConfig {
    pub file: std::path::PathBuf,
    pub level: log::LevelFilter,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum TlsSecurityLevel {
    May,
    Encrypt,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SniKey {
    pub domain: String,
    pub private_key: std::path::PathBuf,
    pub fullchain: std::path::PathBuf,
    pub protocol_version: Option<ProtocolVersionRequirement>,
}

/// Using a wrapping struct for serialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion(pub rustls::ProtocolVersion);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProtocolVersionRequirement(pub Vec<ProtocolVersion>);

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSmtpsConfig {
    pub security_level: TlsSecurityLevel,
    pub protocol_version: ProtocolVersionRequirement,
    pub capath: std::path::PathBuf,
    pub preempt_cipherlist: bool,
    pub fullchain: std::path::PathBuf,
    pub private_key: std::path::PathBuf,
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    pub sni_maps: Option<Vec<SniKey>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSMTPErrorConfig {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
#[serde(deny_unknown_fields)]
pub struct DurationAlias {
    #[serde(with = "humantime_serde")]
    pub alias: std::time::Duration,
}

fn default_rcpt_count_max() -> usize {
    1000
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerSMTPConfig {
    #[serde(default)]
    pub disable_ehlo: bool,
    #[serde(default)]
    #[serde_as(as = "std::collections::HashMap<DisplayFromStr, _>")]
    pub timeout_client: std::collections::HashMap<StateSMTP, DurationAlias>,
    pub error: InnerSMTPErrorConfig,
    #[serde(default = "default_rcpt_count_max")]
    pub rcpt_count_max: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Service {
    #[serde(rename = "shell")]
    UnixShell {
        name: String,
        #[serde(with = "humantime_serde")]
        timeout: std::time::Duration,
        #[serde(default)]
        user: Option<String>,
        #[serde(default)]
        group: Option<String>,
        command: String,
        args: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerRulesConfig {
    #[serde(default = "InnerRulesConfig::default_directory")]
    pub dir: std::path::PathBuf,
    #[serde(default)]
    pub log: InnerUserLogConfig,
    #[serde(default)]
    pub services: Vec<Service>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueConfig {
    pub capacity: Option<usize>,
    pub retry_max: Option<usize>,
    #[serde(with = "humantime_serde", default)]
    pub cron_period: Option<std::time::Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct InnerDeliveryConfig {
    pub spool_dir: std::path::PathBuf,
    #[serde(serialize_with = "crate::config::serializer::ordered_map")]
    pub queues: std::collections::HashMap<String, QueueConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(transparent)]
pub struct Codes {
    #[serde(serialize_with = "crate::config::serializer::ordered_map")]
    pub codes: std::collections::HashMap<SMTPReplyCode, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub server: InnerServerConfig,
    pub log: InnerLogConfig,
    pub smtps: Option<InnerSmtpsConfig>,
    pub smtp: InnerSMTPConfig,
    pub delivery: InnerDeliveryConfig,
    pub rules: InnerRulesConfig,
    pub reply_codes: Codes,
}
