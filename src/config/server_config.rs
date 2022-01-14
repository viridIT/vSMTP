/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
// TODO: have a ConfigBuilder struct
use serde_with::{serde_as, DisplayFromStr};

use crate::{server::ServerVSMTP, smtp::state::StateSMTP};

use super::{
    custom_code::{CustomSMTPCode, SMTPCode},
    default::DEFAULT_CONFIG,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerServerConfig {
    pub addr: std::net::SocketAddr,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerLogConfig {
    pub file: String,
    pub level: std::collections::HashMap<String, log::LevelFilter>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum TlsSecurityLevel {
    None,
    May,
    Encrypt,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SniKey {
    pub domain: String,
    pub private_key: String,
    pub fullchain: String,
    pub protocol_version: Option<ProtocolVersionRequirement>,
}

/// Using a wrapping struct for serialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion(pub rustls::ProtocolVersion);

/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = \"SSLv2\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv2)]);
/// let s = toml::from_str::<S>("v = \"0x0200\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv2)]);
///
/// let s = toml::from_str::<S>("v = \"SSLv3\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv3)]);
/// let s = toml::from_str::<S>("v = \"0x0300\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv3)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.0\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)]);
/// let s = toml::from_str::<S>("v = \"0x0301\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.1\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)]);
/// let s = toml::from_str::<S>("v = \"0x0302\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.2\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]);
/// let s = toml::from_str::<S>("v = \"0x0303\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.3\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]);
/// let s = toml::from_str::<S>("v = \"0x0304\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]);
/// ```
///
/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = [\"TLSv1.1\", \"TLSv1.2\", \"TLSv1.3\"]").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
/// ```
///
/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = \"^TLSv1.1\"").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
///
/// let s = toml::from_str::<S>("v = \">=SSLv3\"").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::SSLv3),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_0),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
///
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersionRequirement(pub Vec<ProtocolVersion>);

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerTlsConfig {
    pub security_level: TlsSecurityLevel,
    pub protocol_version: ProtocolVersionRequirement,
    pub capath: Option<String>,
    pub preempt_cipherlist: bool,
    pub fullchain: Option<String>,
    pub private_key: Option<String>,
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    pub sni_maps: Option<Vec<SniKey>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerSMTPErrorConfig {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerSMTPConfig {
    pub spool_dir: String,
    pub disable_ehlo: bool,
    #[serde_as(as = "std::collections::HashMap<DisplayFromStr, _>")]
    // #[serde(with = "humantime_serde")]
    pub timeout_client: std::collections::HashMap<StateSMTP, String>,
    pub error: InnerSMTPErrorConfig,
    // TODO: ? use serde_as ?
    pub code: Option<SMTPCode>,
    pub rcpt_count_max: Option<usize>,
}

impl InnerSMTPConfig {
    pub fn get_code(&self) -> &CustomSMTPCode {
        match self.code.as_ref() {
            None | Some(SMTPCode::Raw(_)) => {
                panic!("@get_code must be called after a valid conversion from to raw")
            }
            Some(SMTPCode::Serialized(code)) => code.as_ref(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerRulesConfig {
    pub dir: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct QueueConfig {
    pub capacity: Option<usize>,
    pub retry_max: Option<usize>,
    #[serde(with = "humantime_serde", default)]
    pub cron_period: Option<std::time::Duration>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InnerDeliveryConfig {
    pub queue: std::collections::HashMap<String, QueueConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerConfig {
    pub domain: String,
    pub server: InnerServerConfig,
    pub log: InnerLogConfig,
    pub tls: InnerTlsConfig,
    pub smtp: InnerSMTPConfig,
    pub rules: InnerRulesConfig,
    pub delivery: InnerDeliveryConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        DEFAULT_CONFIG.clone()
    }
}

impl ServerConfig {
    pub fn prepare(&mut self) -> &Self {
        self.prepare_inner(false)
    }

    pub fn prepare_default(&mut self) -> &Self {
        self.prepare_inner(true)
    }

    fn prepare_inner(&mut self, prepare_for_default: bool) -> &Self {
        self.smtp.code =
            Some(match &self.smtp.code {
                Some(SMTPCode::Raw(raw)) => SMTPCode::Serialized(Box::new(
                    CustomSMTPCode::from_raw(raw, self, prepare_for_default),
                )),
                None => SMTPCode::Serialized(Box::new(CustomSMTPCode::from_raw(
                    &std::collections::HashMap::<_, _>::new(),
                    self,
                    prepare_for_default,
                ))),
                Some(SMTPCode::Serialized(c)) => SMTPCode::Serialized(c.clone()),
            });
        self
    }

    pub async fn build(mut self) -> ServerVSMTP {
        self.prepare();
        ServerVSMTP::new(std::sync::Arc::new(self))
            .await
            .expect("Failed to create the server")
    }
}
