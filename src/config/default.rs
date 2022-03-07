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
use crate::smtp::code::SMTPReplyCode;

use super::server_config::{
    Codes, InnerLogConfig, InnerSMTPConfig, InnerSMTPErrorConfig, InnerServerConfig,
    InnerUserLogConfig,
};

impl Default for InnerServerConfig {
    fn default() -> Self {
        Self {
            domain: Default::default(),
            vsmtp_user: "vsmtp".to_string(),
            vsmtp_group: "vsmtp".to_string(),
            addr: "0.0.0.0:25".parse().expect("valid address"),
            addr_submission: "0.0.0.0:587".parse().expect("valid address"),
            addr_submissions: "0.0.0.0:465".parse().expect("valid address"),
            thread_count: num_cpus::get(),
        }
    }
}

impl InnerServerConfig {
    pub(crate) fn default_addr() -> std::net::SocketAddr {
        InnerServerConfig::default().addr
    }

    pub(crate) fn default_addr_submission() -> std::net::SocketAddr {
        InnerServerConfig::default().addr_submission
    }

    pub(crate) fn default_addr_submissions() -> std::net::SocketAddr {
        InnerServerConfig::default().addr_submissions
    }
}

impl Default for InnerLogConfig {
    fn default() -> Self {
        Self {
            file: std::path::PathBuf::from_iter(["/", "var", "log", "vsmtp", "app.log"]),
            level: Default::default(),
        }
    }
}

impl InnerLogConfig {
    pub(crate) fn default_file() -> std::path::PathBuf {
        InnerLogConfig::default().file
    }
}

impl Default for InnerUserLogConfig {
    fn default() -> Self {
        Self {
            file: std::path::PathBuf::from_iter(["/", "var", "log", "vsmtp", "rules.log"]),
            level: log::LevelFilter::Warn,
            format: None,
        }
    }
}

impl Default for InnerSMTPErrorConfig {
    fn default() -> Self {
        Self {
            soft_count: 5,
            hard_count: 10,
            delay: std::time::Duration::from_millis(1000),
        }
    }
}

impl Default for InnerSMTPConfig {
    fn default() -> Self {
        Self {
            disable_ehlo: false,
            timeout_client: Default::default(),
            error: Default::default(),
            rcpt_count_max: Self::default_rcpt_count_max(),
            client_count_max: Self::default_client_count_max(),
        }
    }
}

impl InnerSMTPConfig {
    pub(crate) fn default_client_count_max() -> i64 {
        -1
    }

    pub(super) fn default_rcpt_count_max() -> usize {
        1000
    }
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
            SMTPReplyCode::ConnectionMaxReached => "554 Cannot process connection, closing.\r\n",
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

    /// return the message associated with a [SMTPReplyCode].
    ///
    /// panic if the config is ill-formed
    pub fn get(&self, code: &SMTPReplyCode) -> &String {
        self.codes
            .get(code)
            .unwrap_or_else(|| panic!("ill-formed '{:?}'", code))
    }
}
