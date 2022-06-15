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
use crate::{
    config::field::{FieldQueueDelivery, FieldQueueWorking},
    Config,
};
use vsmtp_common::{collection, state::StateSMTP};

#[test]
fn parse() {
    let toml = include_str!("../../../../../../examples/config/secured.toml");
    pretty_assertions::assert_eq!(
        Config::from_toml(toml).unwrap(),
        Config::builder()
            .with_version_str("=1.0.0")
            .unwrap()
            .with_hostname_and_client_count_max(8)
            .with_default_user_and_thread_pool(3, 3, 3)
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_queues(
                "/var/spool/vsmtp",
                FieldQueueWorking { channel_size: 16 },
                FieldQueueDelivery {
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
            .without_auth()
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .with_dns(
                {
                    let mut cfg = trust_dns_resolver::config::ResolverConfig::new();

                    cfg.set_domain(
                        <trust_dns_resolver::Name as std::str::FromStr>::from_str(
                            "example.dns.com",
                        )
                        .unwrap(),
                    );

                    cfg
                },
                crate::field::ResolverOptsWrapper::default()
            )
            .without_virtual_entries()
            .validate()
            .unwrap()
    );
}
