#![allow(clippy::module_name_repetitions)]

use vsmtp_common::{code::SMTPReplyCode, collection, re::strum};

use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigQueueDelivery, ConfigQueueWorking,
        ConfigServer, ConfigServerInterfaces, ConfigServerLogs, ConfigServerQueues,
        ConfigServerSMTP, ConfigServerSMTPError, ConfigServerSMTPTimeoutClient, ConfigServerSystem,
        ConfigServerSystemThreadPool,
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
    pub(crate) fn default_user() -> String {
        match option_env!("CI") {
            Some(_) => "root",
            None => "vsmtp",
        }
        .to_string()
    }

    pub(crate) fn default_group() -> String {
        match option_env!("CI") {
            Some(_) => "root",
            None => "vsmtp",
        }
        .to_string()
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

impl Default for ConfigServerSMTP {
    fn default() -> Self {
        Self {
            rcpt_count_max: Self::default_rcpt_count_max(),
            disable_ehlo: Self::default_disable_ehlo(),
            required_extension: Self::default_required_extension(),
            error: ConfigServerSMTPError::default(),
            timeout_client: ConfigServerSMTPTimeoutClient::default(),
            codes: Self::default_smtp_codes(),
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
        let codes: std::collections::BTreeMap<SMTPReplyCode, &'static str> = collection! {
            SMTPReplyCode::Help => "214 joining us https://viridit.com/support\r\n",
            SMTPReplyCode::Greetings => "220 {domain} Service ready\r\n",
            SMTPReplyCode::Code221 => "221 Service closing transmission channel\r\n",
            SMTPReplyCode::Code250 => "250 Ok\r\n",
            // TODO: find a better way to generate this codes
            SMTPReplyCode::Code250PlainEsmtp =>
                "250-{domain}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250-AUTH GSSAPI LOGIN\r\n250 STARTTLS\r\n",
            // TODO: find a better way to generate this codes
            SMTPReplyCode::Code250SecuredEsmtp =>
                "250-{domain}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250 AUTH GSSAPI PLAIN LOGIN\r\n",
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
            SMTPReplyCode::BadSequence => "503 Bad sequence of commands\r\n",
            SMTPReplyCode::Code504 => "504 Command parameter not implemented\r\n",
            SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first\r\n",
            SMTPReplyCode::Code554 => "554 permanent problems with the remote server\r\n",
            SMTPReplyCode::Code554tls => "554 Command refused due to lack of security\r\n",
            SMTPReplyCode::ConnectionMaxReached => "554 Cannot process connection, closing.\r\n",
            SMTPReplyCode::AuthMechanismNotSupported => "504 5.5.4 Mechanism is not supported\r\n",
            SMTPReplyCode::AuthenticationSucceeded => "235 2.7.0 Authentication succeeded\r\n",
        };

        assert!(
            <SMTPReplyCode as strum::IntoEnumIterator>::iter().all(|i| codes.contains_key(&i)),
            "default SMTPReplyCode are ill-formed "
        );

        codes
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<_>()
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
