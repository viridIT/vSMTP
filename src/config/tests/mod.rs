use crate::{
    config::{
        server_config::{ProtocolVersion, ProtocolVersionRequirement, SniKey},
        service::Service,
    },
    smtp::{code::SMTPReplyCode, state::StateSMTP},
};

use super::server_config::{QueueConfig, ServerConfig, TlsSecurityLevel};

#[test]
fn init() -> anyhow::Result<()> {
    let _config = ServerConfig::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_rfc_port("test.server.com", "root", "root", None)
        .with_logging(
            "./tmp/log",
            std::collections::HashMap::<String, log::LevelFilter>::default(),
        )
        .with_safe_default_smtps(TlsSecurityLevel::May, "dummy", "dummy", None)
        .with_smtp(
            false,
            std::collections::HashMap::<StateSMTP, std::time::Duration>::default(),
            5,
            10,
            std::time::Duration::from_millis(100),
            1000,
            1000,
        )
        .with_delivery(
            "/tmp/spool",
            std::collections::HashMap::<String, QueueConfig>::default(),
        )
        .with_rules("/tmp/re", vec![])
        .with_default_reply_codes()
        .build();

    Ok(())
}

#[test]
fn init_no_smtps() -> anyhow::Result<()> {
    let _config = ServerConfig::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_rfc_port("test.server.com", "root", "root", None)
        .with_logging(
            "./tmp/log",
            std::collections::HashMap::<String, log::LevelFilter>::default(),
        )
        .without_smtps()
        .with_smtp(
            false,
            std::collections::HashMap::<StateSMTP, std::time::Duration>::default(),
            5,
            10,
            std::time::Duration::from_millis(100),
            1000,
            1000,
        )
        .with_delivery(
            "/tmp/spool",
            std::collections::HashMap::<String, QueueConfig>::default(),
        )
        .with_rules("/tmp/re", vec![])
        .with_default_reply_codes()
        .build();
    Ok(())
}

#[test]
fn from_toml_template_simple() -> anyhow::Result<()> {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../template/simple.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("testserver.com", "vsmtp", "vsmtp", None)
            .with_logging(
                "/var/log/vsmtp/app.log",
                crate::collection! {
                    "default".to_string() => log::LevelFilter::Warn
                },
            )
            .without_smtps()
            .with_default_smtp()
            .with_delivery(
                "/var/spool/vsmtp",
                crate::collection! {
                    "working".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deliver".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deferred".to_string() => QueueConfig {
                        capacity: None,
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_rules("/etc/vsmtp/rules", vec![])
            .with_default_reply_codes()
            .build()
            .unwrap()
    );
    Ok(())
}

#[test]
fn from_toml_template_smtps() -> anyhow::Result<()> {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../template/smtps.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server(
                "testserver.com",
                "vsmtp",
                "vsmtp",
                "0.0.0.0:25".parse().expect("valid address"),
                "0.0.0.0:587".parse().expect("valid address"),
                "0.0.0.0:465".parse().expect("valid address"),
                8,
            )
            .with_logging(
                "/var/log/vsmtp/vsmtp.log",
                crate::collection! {
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
            .with_delivery(
                "./tmp/var/spool/vsmtp",
                crate::collection! {
                    "working".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deliver".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deferred".to_string() => QueueConfig {
                        capacity: None,
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_rules("./config/rules", vec![])
            .with_default_reply_codes()
            .build()
            .unwrap()
    );
    Ok(())
}

#[test]
fn from_toml_template_services() -> anyhow::Result<()> {
    assert_eq!(
        ServerConfig::from_toml(include_str!("../template/services.toml")).unwrap(),
        ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("testserver.com", "vsmtp", "vsmtp", None)
            .with_logging(
                "/var/log/vsmtp/app.log",
                crate::collection! {
                    "default".to_string() => log::LevelFilter::Warn
                },
            )
            .without_smtps()
            .with_smtp(
                false,
                crate::collection! {
                    StateSMTP::Helo =>  std::time::Duration::from_millis(100) ,
                    StateSMTP::Data =>  std::time::Duration::from_millis(200) ,
                },
                5,
                10,
                std::time::Duration::from_millis(1000),
                1000,
                1000,
            )
            .with_delivery(
                "/var/spool/vsmtp",
                crate::collection! {
                    "working".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deliver".to_string() => QueueConfig {
                        capacity: Some(32),
                        retry_max: None,
                        cron_period: None
                    },
                    "deferred".to_string() => QueueConfig {
                        capacity: None,
                        retry_max: Some(10),
                        cron_period: Some(std::time::Duration::from_secs(10))
                    },
                },
            )
            .with_rules_and_logging(
                "/etc/vsmtp/rules",
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
            .with_reply_codes(crate::collection! {
                SMTPReplyCode::Code214 => "214 my custom help message\r\n".to_string(),
                SMTPReplyCode::Code220 => "220 {domain} ESMTP Service ready\r\n".to_string(),
            })
            .build()
            .unwrap()
    );
    Ok(())
}
