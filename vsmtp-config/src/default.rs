#![allow(clippy::module_name_repetitions)]

use vsmtp_common::{
    auth::Mechanism,
    code::SMTPReplyCode,
    collection,
    re::{log, strum},
};

use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigQueueDelivery, ConfigQueueWorking,
        ConfigServer, ConfigServerDNS, ConfigServerInterfaces, ConfigServerLogs,
        ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPAuth, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
    },
    Builder, Config, Service,
};

impl Default for Config {
    fn default() -> Self {
        Builder::ensure(Self {
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
        .unwrap()
    }

    pub(crate) fn default_group() -> users::Group {
        users::get_group_by_name(match option_env!("CI") {
            Some(_) => "root",
            None => "vsmtp",
        })
        .unwrap()
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
        }
    }
}

impl ConfigServerLogs {
    pub(crate) fn default_filepath() -> std::path::PathBuf {
        "/var/log/vsmtp/vsmtp.log".into()
    }

    pub(crate) fn default_format() -> String {
        "{d} {l} - ".to_string()
    }

    pub(crate) fn default_level() -> std::collections::BTreeMap<String, log::LevelFilter> {
        collection! {
            "default".to_string() => log::LevelFilter::Warn
        }
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

    pub(crate) fn default_mechanisms() -> Vec<Mechanism> {
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
            SMTPReplyCode::Help => "214 joining us https://viridit.com/support\r\n".to_string(),
            SMTPReplyCode::Greetings => "220 {domain} Service ready\r\n".to_string(),
            SMTPReplyCode::Code221 => "221 Service closing transmission channel\r\n".to_string(),
            SMTPReplyCode::Code250 => "250 Ok\r\n".to_string(),
            SMTPReplyCode::Code250PlainEsmtp =>
                format!("250-{{domain}}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250-AUTH {}\r\n250 STARTTLS\r\n",
                    <Mechanism as strum::IntoEnumIterator>::iter()
                        .filter(|m| !m.must_be_under_tls())
                        .map(String::from)
                        .collect::<Vec<_>>()
                        .join(" ")
                    ),
            SMTPReplyCode::Code250SecuredEsmtp =>
                format!("250-{{domain}}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250 AUTH {}\r\n",
                    <Mechanism as strum::IntoEnumIterator>::iter()
                        .filter(|m| m.must_be_under_tls())
                        .map(String::from)
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            SMTPReplyCode::Code354 => "354 Start mail input; end with <CRLF>.<CRLF>\r\n".to_string(),
            SMTPReplyCode::Code451 => "451 Requested action aborted: local error in processing\r\n".to_string(),
            SMTPReplyCode::Code451Timeout => "451 Timeout - closing connection.\r\n".to_string(),
            SMTPReplyCode::Code451TooManyError => "451 Too many errors from the client\r\n".to_string(),
            SMTPReplyCode::Code452 => "452 Requested action not taken: insufficient system storage\r\n".to_string(),
            SMTPReplyCode::Code452TooManyRecipients =>
                "452 Requested action not taken: to many recipients\r\n".to_string(),
            SMTPReplyCode::Code454 => "454 TLS not available due to temporary reason\r\n".to_string(),
            SMTPReplyCode::Code500 => "500 Syntax error command unrecognized\r\n".to_string(),
            SMTPReplyCode::Code501 => "501 Syntax error in parameters or arguments\r\n".to_string(),
            SMTPReplyCode::Code502unimplemented => "502 Command not implemented\r\n".to_string(),
            SMTPReplyCode::BadSequence => "503 Bad sequence of commands\r\n".to_string(),
            SMTPReplyCode::Code504 => "504 Command parameter not implemented\r\n".to_string(),
            SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first\r\n".to_string(),
            SMTPReplyCode::Code554 => "554 permanent problems with the remote server\r\n".to_string(),
            SMTPReplyCode::Code554tls => "554 Command refused due to lack of security\r\n".to_string(),
            SMTPReplyCode::ConnectionMaxReached => "554 Cannot process connection, closing.\r\n".to_string(),
            SMTPReplyCode::AuthMechanismNotSupported => "504 5.5.4 Mechanism is not supported\r\n".to_string(),
            SMTPReplyCode::AuthSucceeded => "235 2.7.0 Authentication succeeded\r\n".to_string(),
            // 538 5.7.11 (for documentation purpose)
            // 535 (for production)
            SMTPReplyCode::AuthMechanismMustBeEncrypted =>
                "538 5.7.11 Encryption required for requested authentication mechanism\r\n".to_string(),
            SMTPReplyCode::AuthClientMustNotStart =>
                "501 5.7.0 Client must not start with this mechanism\r\n".to_string(),
            SMTPReplyCode::AuthErrorDecode64 => "501 5.5.2 Invalid, not base64\r\n".to_string(),
            SMTPReplyCode::AuthInvalidCredentials => "535 5.7.8 Authentication credentials invalid\r\n".to_string(),
            SMTPReplyCode::AuthClientCanceled => "501 Authentication canceled by clients\r\n".to_string(),
            SMTPReplyCode::AuthRequired => "530 5.7.0 Authentication required\r\n".to_string(),
        };

        assert!(
            <SMTPReplyCode as strum::IntoEnumIterator>::iter().all(|i| codes.contains_key(&i)),
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
            services: std::collections::BTreeMap::<String, Service>::new(),
        }
    }
}

impl ConfigApp {
    pub(crate) fn default_dirpath() -> std::path::PathBuf {
        "/var/spool/vsmtp/app".into()
    }
}

impl Default for ConfigAppVSL {
    fn default() -> Self {
        Self {
            filepath: Self::default_filepath(),
        }
    }
}

impl ConfigAppVSL {
    pub(crate) fn default_filepath() -> std::path::PathBuf {
        "/etc/vsmtp/main.vsl".into()
    }
}

impl Default for ConfigAppLogs {
    fn default() -> Self {
        Self {
            filepath: Self::default_filepath(),
            level: Self::default_level(),
            format: Self::default_format(),
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
}
