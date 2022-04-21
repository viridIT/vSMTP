/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use super::{get_mx_records, Transport};

use anyhow::Context;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::{filter_by_domain_mut, Rcpt},
    re::{anyhow, log},
    transfer::EmailTransferStatus,
};
use vsmtp_config::Config;

/// the email will be forwarded to another mail exchanger via mx record resolution & smtp.
pub struct Deliver<'r> {
    resolver: &'r TokioAsyncResolver,
}

impl<'r> Deliver<'r> {
    /// create a new deliver with a resolver to get data from the distant dns server.
    #[must_use]
    pub const fn new(resolver: &'r TokioAsyncResolver) -> Self {
        Self { resolver }
    }
}

#[async_trait::async_trait]
impl<'r> Transport for Deliver<'r> {
    async fn deliver(
        &mut self,
        config: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::address::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let envelop = super::build_lettre_envelop(from, &to[..])
            .context("failed to build envelop to deliver email")?;

        let mut filtered_rcpt = filter_by_domain_mut(to);

        for (query, rcpt) in &mut filtered_rcpt {
            // getting mx records for a set of recipients.
            let records = match get_mx_records(self.resolver, query).await {
                Ok(records) => records,
                Err(err) => {
                    log::warn!(
                        target: vsmtp_config::log_channel::DELIVER,
                        "failed to deliver email '{}' to '{query}': {err}",
                        metadata.message_id
                    );

                    // could not find any mx records, we just skip all recipient in the group.
                    for rcpt in rcpt.iter_mut() {
                        rcpt.email_status = match rcpt.email_status {
                            EmailTransferStatus::HeldBack(count) => {
                                EmailTransferStatus::HeldBack(count + 1)
                            }
                            _ => EmailTransferStatus::HeldBack(0),
                        };
                    }

                    continue;
                }
            };

            let mut records = records.iter();

            for record in records.by_ref() {
                let host = record.exchange().to_ascii();
                match send_email(config, self.resolver, &host, &envelop, from, content).await {
                    // if a transfer succeeded, we can stop the lookup.
                    Ok(_) => break,
                    Err(err) => log::warn!(
                        target: vsmtp_config::log_channel::DELIVER,
                        "[deliver] failed to send message to '{host}': {err}",
                    ),
                }
            }

            if records.next().is_none() {
                log::error!(
                    target: vsmtp_config::log_channel::DELIVER,
                    "[deliver] no valid mail exchanger found for '{}', check warnings above.",
                    query
                );

                for rcpt in rcpt.iter_mut() {
                    rcpt.email_status = match rcpt.email_status {
                        EmailTransferStatus::HeldBack(count) => {
                            EmailTransferStatus::HeldBack(count + 1)
                        }
                        _ => EmailTransferStatus::HeldBack(0),
                    };
                }
            } else {
                for rcpt in rcpt.iter_mut() {
                    rcpt.email_status = EmailTransferStatus::Sent;
                }
            }
        }

        Ok(())
    }
}

/// send an email using [lettre].
async fn send_email(
    config: &Config,
    resolver: &TokioAsyncResolver,
    target: &str,
    envelop: &lettre::address::Envelope,
    from: &vsmtp_common::address::Address,
    content: &str,
) -> anyhow::Result<()> {
    lettre::AsyncTransport::send_raw(
        // TODO: transport should be cached.
        &crate::transport::build_transport(config, resolver, from, target)?,
        envelop,
        content.as_bytes(),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod test {

    use trust_dns_resolver::TokioAsyncResolver;
    use vsmtp_common::address::Address;
    use vsmtp_config::{Config, ConfigServerDNS};

    use crate::transport::deliver::{get_mx_records, send_email};

    // use super::*;

    #[tokio::test]
    async fn test_get_mx_records() {
        // FIXME: find a way to guarantee that the mx records exists.
        let mut config = Config::default();
        config.server.dns = ConfigServerDNS::System;
        let resolvers = vsmtp_config::build_resolvers(&config).unwrap();
        let dns = resolvers.get(&config.server.domain).unwrap();

        get_mx_records(dns, "google.com")
            .await
            .expect("couldn't find any mx records for google.com");

        assert!(get_mx_records(dns, "invalid_query").await.is_err());
    }

    #[tokio::test]
    async fn test_delivery() {
        let config = Config::default();
        // NOTE: for this to return ok, we would need to setup a test server running locally.
        assert!(send_email(
            &config,
            &TokioAsyncResolver::tokio_from_system_conf().unwrap(),
            "localhost",
            &lettre::address::Envelope::new(
                Some("a@a.a".parse().unwrap()),
                vec!["b@b.b".parse().unwrap()]
            )
            .unwrap(),
            &Address::try_from("a@a.a".to_string()).unwrap(),
            "content"
        )
        .await
        .is_err());
    }
}
