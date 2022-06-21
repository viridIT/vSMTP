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
use vsmtp_common::{collection, re::log, CodeID, Reply, ReplyCode};

use crate::Config;

#[test]
fn parse() {
    let toml = include_str!("../../../../../../examples/config/logging.toml");
    pretty_assertions::assert_eq!(
        Config::from_toml(toml).unwrap(),
        Config::builder()
            .with_version_str(">=1.0.0, <2.0.0")
            .unwrap()
            .with_hostname()
            .with_default_system()
            .with_ipv4_localhost()
            .with_logs_settings(
                "/var/log/vsmtp/vsmtp.log",
                "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
                collection! {
                    "default".to_string() => log::LevelFilter::Warn,
                    "receiver".to_string() => log::LevelFilter::Info,
                    "rule_engine".to_string() => log::LevelFilter::Warn,
                    "delivery".to_string()=> log::LevelFilter::Error,
                    "parser".to_string()=> log::LevelFilter::Trace,
                }
            )
            .with_default_delivery()
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_smtp_codes(collection! {
                // SMTPReplyCode::Help => "214 my custom help message\r\n".to_string(),
                // SMTPReplyCode::Greetings => "220 {domain} ESMTP Service ready\r\n".to_string(),
                CodeID::Help => Reply::new(ReplyCode::Code{ code: 214 },
                    "This server supports the following commands\nHELO EHLO STARTTLS RCPT DATA RSET MAIL QUIT HELP AUTH"
                        .to_string()),
                CodeID::Greetings => Reply::parse_str("220 {domain} ESMTP Service ready").unwrap(),
                CodeID::TlsRequired => Reply::new(
                    ReplyCode::Enhanced{code: 451, enhanced: "5.7.3".to_string() }, "STARTTLS is required to send mail"
                )
            })
            .without_auth()
            .with_default_app()
            .with_default_vsl_settings()
            .with_app_logs_level_and_format(
                "/var/log/vsmtp/app.log",
                log::LevelFilter::Trace,
                "{d} - {m}{n}",
                20_971_520,
                100,
            )
            .with_system_dns()
            .without_virtual_entries()
            .validate()
            .unwrap()
    );
}
