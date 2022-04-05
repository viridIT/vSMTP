use crate::config::ConfigServerDNS;
use crate::config::ConfigServerSMTPAuth;
use vsmtp_common::re::log;

use super::config::{
    Config, ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigQueueDelivery, ConfigQueueWorking,
    ConfigServer, ConfigServerInterfaces, ConfigServerLogs, ConfigServerQueues, ConfigServerSMTP,
    ConfigServerSMTPError, ConfigServerSMTPTimeoutClient, ConfigServerSystem,
    ConfigServerSystemThreadPool, ConfigServerTls, TlsSecurityLevel,
};

mod root_example {
    mod antivirus;
    mod logging;
    mod minimal;
    mod secured;
    mod simple;
    mod tls;
}

mod validate;

#[test]
fn construct() {
    let _c = Config {
        version_requirement: semver::VersionReq::STAR,
        server: ConfigServer {
            domain: "domain.com".to_string(),
            client_count_max: 100,
            system: ConfigServerSystem {
                user: users::get_user_by_name("root").unwrap(),
                group: users::get_group_by_name("root").unwrap(),
                thread_pool: ConfigServerSystemThreadPool {
                    receiver: 6,
                    processing: 6,
                    delivery: 6,
                },
            },
            interfaces: ConfigServerInterfaces {
                addr: vec!["0.0.0.0:25".parse().expect("valid")],
                addr_submission: vec!["0.0.0.0:587".parse().expect("valid")],
                addr_submissions: vec!["0.0.0.0:465".parse().expect("valid")],
            },
            logs: ConfigServerLogs {
                filepath: "/var/log/vsmtp/vsmtp.log".into(),
                format: "{d} {l} - ".to_string(),
                level: std::collections::BTreeMap::new(),
            },
            queues: ConfigServerQueues {
                dirpath: "/var/spool/vsmtp".into(),
                working: ConfigQueueWorking { channel_size: 12 },
                delivery: ConfigQueueDelivery {
                    channel_size: 12,
                    deferred_retry_max: 100,
                    deferred_retry_period: std::time::Duration::from_millis(30_000),
                    // dead_file_lifetime: (),
                },
            },
            tls: Some(ConfigServerTls {
                security_level: TlsSecurityLevel::May,
                preempt_cipherlist: false,
                handshake_timeout: std::time::Duration::from_millis(200),
                protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
                certificate: rustls::Certificate(vec![]),
                private_key: rustls::PrivateKey(vec![]),
                sni: vec![],
                cipher_suite: vec![],
            }),
            smtp: ConfigServerSMTP {
                rcpt_count_max: 1000,
                disable_ehlo: false,
                required_extension: vec![],
                error: ConfigServerSMTPError {
                    soft_count: 5,
                    hard_count: 10,
                    delay: std::time::Duration::from_millis(500),
                },
                timeout_client: ConfigServerSMTPTimeoutClient {
                    connect: std::time::Duration::from_secs(5 * 60),
                    helo: std::time::Duration::from_secs(5 * 60),
                    mail_from: std::time::Duration::from_secs(5 * 60),
                    rcpt_to: std::time::Duration::from_secs(5 * 60),
                    data: std::time::Duration::from_secs(5 * 60),
                },
                codes: std::collections::BTreeMap::new(),
                auth: Some(ConfigServerSMTPAuth {
                    enable_dangerous_mechanism_in_clair: false,
                    mechanisms: vec![],
                    attempt_count_max: -1,
                    must_be_authenticated: false,
                }),
            },
            dns: ConfigServerDNS::default(),
        },
        app: ConfigApp {
            dirpath: "/var/spool/vsmtp/app".into(),
            vsl: ConfigAppVSL {
                filepath: "/etc/vsmtp/rules/main.vsl".into(),
            },
            logs: ConfigAppLogs {
                filepath: "/var/log/vsmtp/app.log".into(),
                level: log::LevelFilter::Info,
                format: "{d} - {m}{n}".to_string(),
            },
            services: std::collections::BTreeMap::new(),
        },
    };
}
