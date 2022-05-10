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

use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigQueueDelivery, ConfigQueueWorking,
        ConfigServer, ConfigServerDNS, ConfigServerInterfaces, ConfigServerLogs,
        ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPAuth, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
    },
    Config, ConfigServerTls, ConfigServerVirtualTls, ResolverOptsWrapper, TlsSecurityLevel,
};
use vsmtp_common::{
    auth::Mechanism,
    code::SMTPReplyCode,
    collection,
    re::{log, strum},
};

impl Default for Config {
    fn default() -> Self {
        Self::ensure(Self {
            version_requirement: semver::VersionReq::parse("<1.0.0").unwrap(),
            server: ConfigServer::default(),
            app: ConfigApp::default(),
        })
        .unwrap()
    }
}

impl Default for ConfigServer {
    fn default() -> Self {
        Self {
            domain: Self::hostname(),
            client_count_max: Self::default_client_count_max(),
            system: ConfigServerSystem::default(),
            interfaces: ConfigServerInterfaces::default(),
            logs: ConfigServerLogs::default(),
            queues: ConfigServerQueues::default(),
            tls: None,
            smtp: ConfigServerSMTP::default(),
            dns: ConfigServerDNS::default(),
            r#virtual: std::collections::BTreeMap::default(),
        }
    }
}

impl ConfigServer {
    pub(crate) fn hostname() -> String {
        hostname::get().unwrap().to_str().unwrap().to_string()
    }

    pub(crate) const fn default_client_count_max() -> i64 {
        16
    }
}

impl Default for ConfigServerSystem {
    fn default() -> Self {
        Self {
            user: Self::default_user(),
            group: Self::default_group(),
            group_local: None,
            thread_pool: ConfigServerSystemThreadPool::default(),
        }
    }
}

impl ConfigServerSystem {
    pub(crate) fn default_user() -> users::User {
        users::get_user_by_name(match option_env!("CI") {
            Some(_) => "root",
            None => "vsmtp",
        })
        .expect("user 'vsmtp' not found")
    }

    pub(crate) fn default_group() -> users::Group {
        users::get_group_by_name(match option_env!("CI") {
            Some(_) => "root",
            None => "vsmtp",
        })
        .expect("user 'vsmtp' not found")
    }
}

impl Default for ConfigServerSystemThreadPool {
    fn default() -> Self {
        Self {
            receiver: Self::default_receiver(),
            processing: Self::default_processing(),
            delivery: Self::default_delivery(),
        }
    }
}

impl ConfigServerSystemThreadPool {
    pub(crate) const fn default_receiver() -> usize {
        6
    }

    pub(crate) const fn default_processing() -> usize {
        6
    }

    pub(crate) const fn default_delivery() -> usize {
        6
    }
}

impl Default for ConfigServerInterfaces {
    fn default() -> Self {
        Self::ipv4_localhost()
    }
}

impl ConfigServerInterfaces {
    pub(crate) fn ipv4_localhost() -> Self {
        Self {
            addr: vec!["127.0.0.1:25".parse().expect("valid")],
            addr_submission: vec!["127.0.0.1:587".parse().expect("valid")],
            addr_submissions: vec!["127.0.0.1:465".parse().expect("valid")],
        }
    }
}

impl Default for ConfigServerLogs {
    fn default() -> Self {
        Self {
            filepath: Self::default_filepath(),
            format: Self::default_format(),
            level: Self::default_level(),
            size_limit: Self::default_size_limit(),
            archive_count: Self::default_archive_count(),
        }
    }
}

impl ConfigServerLogs {
    pub(crate) fn default_filepath() -> std::path::PathBuf {
        "/var/log/vsmtp/vsmtp.log".into()
    }

    pub(crate) fn default_format() -> String {
        "{d(%Y-%m-%d %H:%M:%S%.f)} {h({l:<5} [{I}])} {t:<30} $ {m}{n}".to_string()
    }

    pub(crate) fn default_level() -> std::collections::BTreeMap<String, log::LevelFilter> {
        collection! {
            "default".to_string() => log::LevelFilter::Warn
        }
    }

    pub(crate) const fn default_size_limit() -> u64 {
        10_485_760 // 10MB
    }

    pub(crate) const fn default_archive_count() -> u32 {
        10
    }
}

impl ConfigServerTls {
    pub(crate) fn default_cipher_suite() -> Vec<rustls::CipherSuite> {
        vec![
            // TLS1.3 suites
            rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS13_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
            // TLS1.2 suites
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        ]
    }
}

impl Default for ConfigServerQueues {
    fn default() -> Self {
        Self {
            dirpath: Self::default_dirpath(),
            working: ConfigQueueWorking::default(),
            delivery: ConfigQueueDelivery::default(),
        }
    }
}

impl ConfigServerQueues {
    pub(crate) fn default_dirpath() -> std::path::PathBuf {
        "/var/spool/vsmtp".into()
    }
}

impl Default for ConfigQueueWorking {
    fn default() -> Self {
        Self { channel_size: 32 }
    }
}

impl Default for ConfigQueueDelivery {
    fn default() -> Self {
        Self {
            channel_size: 32,
            deferred_retry_max: 100,
            deferred_retry_period: std::time::Duration::from_secs(300),
        }
    }
}

impl ConfigServerVirtualTls {
    pub(crate) const fn default_sender_security_level() -> TlsSecurityLevel {
        TlsSecurityLevel::Encrypt
    }
}

impl Default for ConfigServerSMTPAuth {
    fn default() -> Self {
        Self {
            enable_dangerous_mechanism_in_clair: Self::default_enable_dangerous_mechanism_in_clair(
            ),
            mechanisms: Self::default_mechanisms(),
            attempt_count_max: Self::default_attempt_count_max(),
            must_be_authenticated: Self::default_must_be_authenticated(),
        }
    }
}

impl ConfigServerSMTPAuth {
    pub(crate) const fn default_enable_dangerous_mechanism_in_clair() -> bool {
        false
    }

    /// Return all the supported SASL mechanisms
    #[must_use]
    pub fn default_mechanisms() -> Vec<Mechanism> {
        <Mechanism as strum::IntoEnumIterator>::iter().collect::<Vec<_>>()
    }

    pub(crate) const fn default_attempt_count_max() -> i64 {
        -1
    }

    pub(crate) const fn default_must_be_authenticated() -> bool {
        false
    }
}

impl Default for ConfigServerSMTP {
    fn default() -> Self {
        Self {
            rcpt_count_max: Self::default_rcpt_count_max(),
            disable_ehlo: Self::default_disable_ehlo(),
            required_extension: Self::default_required_extension(),
            error: ConfigServerSMTPError::default(),
            timeout_client: ConfigServerSMTPTimeoutClient::default(),
            codes: Self::default_smtp_codes(),
            auth: None,
        }
    }
}

impl ConfigServerSMTP {
    pub(crate) const fn default_rcpt_count_max() -> usize {
        1000
    }

    pub(crate) const fn default_disable_ehlo() -> bool {
        false
    }

    pub(crate) fn default_required_extension() -> Vec<String> {
        ["STARTTLS", "SMTPUTF8", "8BITMIME", "AUTH"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    pub(crate) fn default_smtp_codes() -> std::collections::BTreeMap<SMTPReplyCode, String> {
        let codes: std::collections::BTreeMap<SMTPReplyCode, String> = collection! {
            SMTPReplyCode::Help => "214 joining us https://viridit.com/support".to_string(),
            SMTPReplyCode::Greetings => "220 {domain} Service ready".to_string(),
            SMTPReplyCode::Code221 => "221 Service closing transmission channel".to_string(),
            SMTPReplyCode::Code250 => "250 Ok".to_string(),
            SMTPReplyCode::Code354 => "354 Start mail input; end with <CRLF>.<CRLF>".to_string(),
            SMTPReplyCode::Code451 => "451 Requested action aborted: local error in processing".to_string(),
            SMTPReplyCode::Code451Timeout => "451 Timeout - closing connection.".to_string(),
            SMTPReplyCode::Code451TooManyError => "451 Too many errors from the client".to_string(),
            SMTPReplyCode::Code452 => "452 Requested action not taken: insufficient system storage".to_string(),
            SMTPReplyCode::Code452TooManyRecipients =>
                "452 Requested action not taken: to many recipients".to_string(),
            SMTPReplyCode::Code454 => "454 TLS not available due to temporary reason".to_string(),
            SMTPReplyCode::Code500 => "500 Syntax error command unrecognized".to_string(),
            SMTPReplyCode::Code501 => "501 Syntax error in parameters or arguments".to_string(),
            SMTPReplyCode::Code502unimplemented => "502 Command not implemented".to_string(),
            SMTPReplyCode::BadSequence => "503 Bad sequence of commands".to_string(),
            SMTPReplyCode::Code504 => "504 Command parameter not implemented".to_string(),
            SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first".to_string(),
            SMTPReplyCode::Code554 => "554 permanent problems with the remote server".to_string(),
            SMTPReplyCode::Code554tls => "554 Command refused due to lack of security".to_string(),
            SMTPReplyCode::TlsAlreadyUnderTls => "554 5.5.1 Error: TLS already active".to_string(),
            SMTPReplyCode::ConnectionMaxReached => "554 Cannot process connection, closing.".to_string(),
            SMTPReplyCode::AuthMechanismNotSupported => "504 5.5.4 Mechanism is not supported".to_string(),
            SMTPReplyCode::AuthSucceeded => "235 2.7.0 Authentication succeeded".to_string(),
            // 538 5.7.11 (for documentation purpose)
            // 535 (for production)
            SMTPReplyCode::AuthMechanismMustBeEncrypted =>
                "538 5.7.11 Encryption required for requested authentication mechanism".to_string(),
            SMTPReplyCode::AuthClientMustNotStart =>
                "501 5.7.0 Client must not start with this mechanism".to_string(),
            SMTPReplyCode::AuthErrorDecode64 => "501 5.5.2 Invalid, not base64".to_string(),
            SMTPReplyCode::AuthInvalidCredentials => "535 5.7.8 Authentication credentials invalid".to_string(),
            SMTPReplyCode::AuthClientCanceled => "501 Authentication canceled by clients".to_string(),
            SMTPReplyCode::AuthRequired => "530 5.7.0 Authentication required".to_string(),
            SMTPReplyCode::Custom(String::default()) => String::default(),
        };

        assert!(
            <SMTPReplyCode as strum::IntoEnumIterator>::iter()
                // exclude these codes because they are generated by the [Config::ensure] and vsl.
                .filter(|i| ![
                    SMTPReplyCode::Code250PlainEsmtp,
                    SMTPReplyCode::Code250SecuredEsmtp,
                ]
                .contains(i))
                .all(|i| codes.contains_key(&i)),
            "default SMTPReplyCode are ill-formed "
        );

        codes
    }
}

impl Default for ConfigServerDNS {
    fn default() -> Self {
        Self::System
    }
}

impl Default for ResolverOptsWrapper {
    fn default() -> Self {
        Self {
            timeout: Self::default_timeout(),
            attempts: Self::default_attempts(),
            rotate: Self::default_rotate(),
            dnssec: Self::default_dnssec(),
            ip_strategy: Self::default_ip_strategy(),
            cache_size: Self::default_cache_size(),
            use_hosts_file: Self::default_use_hosts_file(),
            num_concurrent_reqs: Self::default_num_concurrent_reqs(),
        }
    }
}

impl ResolverOptsWrapper {
    pub(crate) const fn default_timeout() -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }

    pub(crate) const fn default_attempts() -> usize {
        2
    }
    pub(crate) const fn default_rotate() -> bool {
        false
    }

    pub(crate) const fn default_dnssec() -> bool {
        false
    }

    pub(crate) fn default_ip_strategy() -> trust_dns_resolver::config::LookupIpStrategy {
        trust_dns_resolver::config::LookupIpStrategy::default()
    }

    pub(crate) const fn default_cache_size() -> usize {
        32
    }

    pub(crate) const fn default_use_hosts_file() -> bool {
        true
    }

    pub(crate) const fn default_num_concurrent_reqs() -> usize {
        2
    }
}

impl Default for ConfigServerSMTPError {
    fn default() -> Self {
        Self {
            soft_count: 10,
            hard_count: 20,
            delay: std::time::Duration::from_millis(5000),
        }
    }
}

impl Default for ConfigServerSMTPTimeoutClient {
    fn default() -> Self {
        Self {
            connect: std::time::Duration::from_secs(5 * 60),
            helo: std::time::Duration::from_secs(5 * 60),
            mail_from: std::time::Duration::from_secs(5 * 60),
            rcpt_to: std::time::Duration::from_secs(5 * 60),
            data: std::time::Duration::from_secs(5 * 60),
        }
    }
}

impl Default for ConfigApp {
    fn default() -> Self {
        Self {
            dirpath: Self::default_dirpath(),
            vsl: ConfigAppVSL::default(),
            logs: ConfigAppLogs::default(),
        }
    }
}

impl ConfigApp {
    pub(crate) fn default_dirpath() -> std::path::PathBuf {
        "/var/spool/vsmtp/app".into()
    }
}

impl Default for ConfigAppLogs {
    fn default() -> Self {
        Self {
            filepath: Self::default_filepath(),
            level: Self::default_level(),
            format: Self::default_format(),
            size_limit: Self::default_size_limit(),
            archive_count: Self::default_archive_count(),
        }
    }
}

impl ConfigAppLogs {
    pub(crate) fn default_filepath() -> std::path::PathBuf {
        "/var/log/vsmtp/app.log".into()
    }

    pub(crate) const fn default_level() -> log::LevelFilter {
        log::LevelFilter::Warn
    }

    pub(crate) fn default_format() -> String {
        "{d} - {m}{n}".to_string()
    }

    pub(crate) const fn default_size_limit() -> u64 {
        10_485_760 // 10MB
    }

    pub(crate) const fn default_archive_count() -> u32 {
        10
    }
}
