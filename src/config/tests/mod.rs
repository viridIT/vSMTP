use pretty_assertions::assert_eq;

use crate::{
    collection,
    config::{
        server_config::{InnerQueuesConfig, ProtocolVersion, ProtocolVersionRequirement, SniKey},
        service::Service,
    },
    smtp::{code::SMTPReplyCode, state::StateSMTP},
};

use super::server_config::{QueueConfig, ServerConfig, TlsSecurityLevel};

#[test]
fn simple() {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../../../examples/config/simple.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("testserver.com", "root", "root", None)
            .with_logging(
                "/var/log/vsmtp/app.log",
                collection! {
                    "default".to_string() => log::LevelFilter::Warn
                },
            )
            .without_smtps()
            .with_default_smtp()
            .with_delivery_and_queues(
                "/var/spool/vsmtp",
                "none",
                InnerQueuesConfig {
                    working: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deliver: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deferred: QueueConfig {
                        capacity: QueueConfig::default_capacity(),
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_empty_rules()
            .with_default_reply_codes()
            .build()
            .unwrap()
    );
}

#[test]
fn smtps() {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../../../examples/config/smtps.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server(
                "testserver.com",
                "root",
                "root",
                "0.0.0.0:25".parse().expect("valid address"),
                "0.0.0.0:587".parse().expect("valid address"),
                "0.0.0.0:465".parse().expect("valid address"),
                8,
            )
            .with_logging(
                "/var/log/vsmtp/vsmtp.log",
                collection! {
                    "default".to_string() => log::LevelFilter::Warn
                },
            )
            .with_smtps(
                TlsSecurityLevel::May,
                ProtocolVersionRequirement(vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]),
                "./config/certs",
                true,
                "{capath}/certificate.crt",
                "{capath}/privateKey.key",
                std::time::Duration::from_millis(100),
                Some(vec![SniKey {
                    domain: "testserver.com".to_string(),
                    private_key: "{capath}/rsa.{domain}.pem".into(),
                    fullchain: "{capath}/fullchain.{domain}.pem".into(),
                    protocol_version: None
                }]),
            )
            .with_default_smtp()
            .with_delivery_and_queues(
                "./tmp/var/spool/vsmtp",
                "none",
                InnerQueuesConfig {
                    working: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deliver: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deferred: QueueConfig {
                        capacity: QueueConfig::default_capacity(),
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_empty_rules()
            .with_default_reply_codes()
            .build()
            .unwrap()
    );
}

#[test]
fn services() {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../../../examples/config/services.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("testserver.com", "root", "root", None)
            .with_logging(
                "/var/log/vsmtp/app.log",
                collection! {
                    "default".to_string() => log::LevelFilter::Warn
                },
            )
            .without_smtps()
            .with_smtp(
                false,
                collection! {
                    StateSMTP::Helo =>  std::time::Duration::from_millis(100) ,
                    StateSMTP::Data =>  std::time::Duration::from_millis(200) ,
                },
                5,
                10,
                std::time::Duration::from_millis(1000),
                1000,
                -1,
            )
            .with_delivery_and_queues(
                "/var/spool/vsmtp",
                "none",
                InnerQueuesConfig {
                    working: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deliver: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deferred: QueueConfig {
                        capacity: QueueConfig::default_capacity(),
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_rules_and_logging(
                "/etc/vsmtp/rules/main.vsl",
                vec![
                    Service::UnixShell {
                        name: "echo_hello".to_string(),
                        command: "echo".to_string(),
                        timeout: std::time::Duration::from_millis(500),
                        args: Some("hello".to_string()),
                        user: None,
                        group: None
                    },
                    Service::UnixShell {
                        name: "anti_spam".to_string(),
                        command: "/usr/bin/anti_spam".to_string(),
                        timeout: std::time::Duration::from_millis(1500),
                        args: Some("hello".to_string()),
                        user: Some("anti_spam".to_string()),
                        group: Some("anti_spam".to_string())
                    }
                ],
                "/var/log/vsmtp/custom_file.log",
                log::LevelFilter::Trace,
                Some("{d} - {m}{n}".to_string()),
            )
            .with_reply_codes(collection! {
                SMTPReplyCode::Code214 => "214 my custom help message\r\n".to_string(),
                SMTPReplyCode::Code220 => "220 {domain} ESMTP Service ready\r\n".to_string(),
            })
            .build()
            .unwrap()
    );
}

#[allow(clippy::too_many_lines)]
#[test]
fn complete() {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../../../examples/config/complete.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str(">=0.9.2, <1.0.0")
            .unwrap()
            .with_server(
                "testserver.com",
                "root",
                "root",
                "0.0.0.0:10025".parse().unwrap(),
                "0.0.0.0:10587".parse().unwrap(),
                "0.0.0.0:10465".parse().unwrap(),
                10,
            )
            .with_logging(
                "/var/log/vsmtp/vsmtp.log",
                collection! {
                    "default".to_string() => log::LevelFilter::Warn,
                    "receiver".to_string() => log::LevelFilter::Info,
                    "resolver".to_string() => log::LevelFilter::Error,
                    "rules".to_string() => log::LevelFilter::Warn,
                }
            )
            .with_smtps(
                TlsSecurityLevel::May,
                ProtocolVersionRequirement(vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]),
                "./config/certs",
                true,
                "{capath}/certificate.crt",
                "{capath}/privateKey.key",
                std::time::Duration::from_millis(100),
                Some(vec![
                    SniKey {
                        domain: "testserver.com".to_string(),
                        private_key: "{capath}/rsa.{domain}.pem".into(),
                        fullchain: "{capath}/fullchain.{domain}.pem".into(),
                        protocol_version: None
                    },
                    SniKey {
                        domain: "testserver2.com".to_string(),
                        private_key: "{capath}/rsa.{domain}.pem".into(),
                        fullchain: "{capath}/fullchain.{domain}.pem".into(),
                        protocol_version: None
                    }
                ]),
            )
            .with_smtp(
                false,
                collection! {
                    StateSMTP::Connect => std::time::Duration::from_millis(50),
                    StateSMTP::Helo => std::time::Duration::from_millis (100),
                    StateSMTP::MailFrom => std::time::Duration::from_millis (200),
                    StateSMTP::RcptTo => std::time::Duration::from_millis (400),
                    StateSMTP::Data => std::time::Duration::from_millis (800),
                },
                5,
                10,
                std::time::Duration::from_millis(5000),
                25,
                16
            )
            .with_delivery_and_queues(
                "/var/spool/vsmtp",
                "none",
                InnerQueuesConfig {
                    working: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deliver: QueueConfig {
                        capacity: 32,
                        retry_max: None,
                        cron_period: None
                    },
                    deferred: QueueConfig {
                        capacity: 32,
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_rules(
                "/etc/vsmtp/rules/main.vsl",
                vec![
                    Service::UnixShell {
                        name: "clamscan".to_string(),
                        timeout: std::time::Duration::from_millis(15000),
                        user: None,
                        group: None,
                        command: "/etc/vsmtp/rules/service/clamscan.sh".to_string(),
                        args: Some("{mail}".to_string())
                    },
                    Service::UnixShell {
                        name: "spamassassin".to_string(),
                        timeout: std::time::Duration::from_millis(15000),
                        user: Some("root".to_string()),
                        group: Some("root".to_string()),
                        command: "/etc/vsmtp/rules/service/spamassassin.sh".to_string(),
                        args: Some("{mail}".to_string())
                    }
                ],
            )
            .with_reply_codes(collection! {
                SMTPReplyCode::Code214 => "214 my custom help message\r\n".to_string(),
                SMTPReplyCode::Code220 => "220 {domain} ESMTP Service ready\r\n".to_string(),
            })
            .build()
            .unwrap()
    );
}
