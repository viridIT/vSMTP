use vsmtp_common::{collection, state::StateSMTP};

use crate::{
    config::{ConfigQueueDelivery, ConfigQueueWorking},
    Config,
};

#[test]
fn parse() {
    let toml = include_str!("../../../../examples/config/secured.toml");
    pretty_assertions::assert_eq!(
        Config::from_toml(toml).unwrap(),
        Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_hostname_and_client_count_max(8)
            .with_default_user_and_thread_pool(3, 3, 3)
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_queues(
                "/var/spool/vsmtp",
                ConfigQueueWorking { channel_size: 16 },
                ConfigQueueDelivery {
                    channel_size: 16,
                    deferred_retry_max: 10,
                    deferred_retry_period: std::time::Duration::from_secs(600)
                }
            )
            .without_tls_support()
            .with_rcpt_count_and_default(25)
            .with_error_handler_and_timeout(
                5,
                10,
                std::time::Duration::from_millis(50_000),
                &collection! {
                    StateSMTP::Connect => std::time::Duration::from_millis(50),
                    StateSMTP::Helo => std::time::Duration::from_millis(100),
                    StateSMTP::MailFrom => std::time::Duration::from_millis(200),
                    StateSMTP::RcptTo => std::time::Duration::from_millis(400),
                    StateSMTP::Data => std::time::Duration::from_millis(800),
                }
            )
            .with_default_smtp_codes()
            .with_app_at_location("/var/spool/vsmtp/app")
            .with_default_vsl_settings()
            .with_default_app_logs()
            .without_services()
            .validate()
            .unwrap()
    );
}
