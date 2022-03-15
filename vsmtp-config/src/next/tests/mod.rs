use crate::TlsSecurityLevel;

use super::{
    Config, ConfigQueueDelivery, ConfigQueueWorking, ConfigServer, ConfigServerInterfaces,
    ConfigServerLogs, ConfigServerQueues, ConfigServerSystem, ConfigServerSystemThreadPool,
    ConfigServerTls,
};

#[test]
fn serialize() {
    let c = Config {
        version_requirement: semver::VersionReq::STAR,
        server: ConfigServer {
            domain: "domain.com".to_string(),
            client_count_max: 100,
            system: ConfigServerSystem {
                user: "vsmtp".to_string(),
                group: "vsmtp".to_string(),
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
            tls: ConfigServerTls {
                security_level: TlsSecurityLevel::May,
                preempt_cipherlist: false,
                handshake_timeout: std::time::Duration::from_millis(200),
                protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
                certificate: rustls::Certificate(vec![]),
                private_key: rustls::PrivateKey(vec![]),
                sni: vec![],
            },
        },
    };

    let mut fs = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("trace.json")
        .unwrap();
    std::io::Write::write_all(
        &mut fs,
        serde_json::to_string_pretty(&c).unwrap().as_bytes(),
    )
    .unwrap();
}
