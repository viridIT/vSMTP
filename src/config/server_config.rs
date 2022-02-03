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
use serde_with::{serde_as, DisplayFromStr};

use crate::smtp::{code::SMTPReplyCode, state::StateSMTP};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerServerConfig {
    pub domain: String,
    pub addr: std::net::SocketAddr,
    pub addr_submission: std::net::SocketAddr,
    pub addr_submissions: std::net::SocketAddr,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerLogConfig {
    pub file: String,
    pub level: std::collections::HashMap<String, log::LevelFilter>,
}

#[derive(Debug, Copy, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TlsSecurityLevel {
    None,
    May,
    Encrypt,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SniKey {
    pub domain: String,
    pub private_key: String,
    pub fullchain: String,
    pub protocol_version: Option<ProtocolVersionRequirement>,
}

/// Using a wrapping struct for serialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion(pub rustls::ProtocolVersion);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProtocolVersionRequirement(pub Vec<ProtocolVersion>);

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPErrorConfig {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct DurationAlias {
    #[serde(with = "humantime_serde")]
    pub alias: std::time::Duration,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPConfig {
    pub disable_ehlo: bool,
    #[serde(default)]
    #[serde_as(as = "std::collections::HashMap<DisplayFromStr, _>")]
    pub timeout_client: std::collections::HashMap<StateSMTP, DurationAlias>,
    pub error: InnerSMTPErrorConfig,
    pub rcpt_count_max: usize,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerRulesConfig {
    pub dir: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QueueConfig {
    pub capacity: Option<usize>,
    pub retry_max: Option<usize>,
    #[serde(with = "humantime_serde", default)]
    pub cron_period: Option<std::time::Duration>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InnerDeliveryConfig {
    pub spool_dir: String,
    pub queues: std::collections::HashMap<String, QueueConfig>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct Codes {
    pub codes: std::collections::HashMap<SMTPReplyCode, String>,
}

impl Default for Codes {
    fn default() -> Self {
        let codes: std::collections::HashMap<SMTPReplyCode, &'static str> = crate::collection! {
            SMTPReplyCode::Code214 => "214 joining us https://viridit.com/support\r\n",
            SMTPReplyCode::Code220 => "220 {domain} Service ready\r\n",
            SMTPReplyCode::Code221 => "221 Service closing transmission channel\r\n",
            SMTPReplyCode::Code250 => "250 Ok\r\n",
            SMTPReplyCode::Code250PlainEsmtp => "250-{domain}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250 STARTTLS\r\n",
            SMTPReplyCode::Code250SecuredEsmtp => "250-{domain}\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n",
            SMTPReplyCode::Code354 => "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            SMTPReplyCode::Code451 => "451 Requested action aborted: local error in processing\r\n",
            SMTPReplyCode::Code451Timeout => "451 Timeout - closing connection.\r\n",
            SMTPReplyCode::Code451TooManyError => "451 Too many errors from the client\r\n",
            SMTPReplyCode::Code452 => "452 Requested action not taken: insufficient system storage\r\n",
            SMTPReplyCode::Code452TooManyRecipients => "452 Requested action not taken: to many recipients\r\n",
            SMTPReplyCode::Code454 => "454 TLS not available due to temporary reason\r\n",
            SMTPReplyCode::Code500 => "500 Syntax error command unrecognized\r\n",
            SMTPReplyCode::Code501 => "501 Syntax error in parameters or arguments\r\n",
            SMTPReplyCode::Code502unimplemented => "502 Command not implemented\r\n",
            SMTPReplyCode::Code503 => "503 Bad sequence of commands\r\n",
            SMTPReplyCode::Code504 => "504 Command parameter not implemented\r\n",
            SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first\r\n",
            SMTPReplyCode::Code554 => "554 permanent problems with the remote server\r\n",
            SMTPReplyCode::Code554tls => "554 Command refused due to lack of security\r\n",
        };

        let out = Self {
            codes: codes
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect::<_>(),
        };
        assert!(out.is_not_ill_formed(), "missing codes in default values");
        out
    }
}

impl Codes {
    fn is_not_ill_formed(&self) -> bool {
        <SMTPReplyCode as enum_iterator::IntoEnumIterator>::into_enum_iter()
            .all(|i| self.codes.contains_key(&i))
    }

    pub fn get(&self, code: &SMTPReplyCode) -> &String {
        self.codes
            .get(code)
            .unwrap_or_else(|| panic!("ill-formed '{:?}'", code))
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub server: InnerServerConfig,
    pub log: InnerLogConfig,
    pub tls: Option<InnerTlsConfig>,
    pub smtp: InnerSMTPConfig,
    pub delivery: InnerDeliveryConfig,
    pub rules: InnerRulesConfig,
    pub reply_codes: Codes,
}
