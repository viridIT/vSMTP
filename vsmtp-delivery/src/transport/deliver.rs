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
use super::Transport;

use anyhow::Context;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{anyhow, log},
    transfer::EmailTransferStatus,
};
use vsmtp_config::Config;

/// the email will be forwarded to another mail exchanger via mx record resolution & smtp.
#[derive(Default)]
pub struct Deliver;

#[async_trait::async_trait]
impl Transport for Deliver {
    async fn deliver(
        &mut self,
        config: &Config,
        dns: &TokioAsyncResolver,
        metadata: &MessageMetadata,
        from: &vsmtp_common::address::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let envelop = super::build_lettre_envelop(from, &to[..])
            .context("failed to build envelop to deliver email")?;

        let mut to = rcpt_by_domain(to);

        for (query, rcpt) in &mut to {
            // getting mx records for a set of recipients.
            let records = match get_mx_records(dns, query).await {
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

            // we try to deliver the email to the recipients of the current group using found mail exchangers.
            for record in records.by_ref() {
                for destination in dns.lookup_ip(&record.exchange().to_ascii()).await?.iter() {
                    if (send_email(config, &destination.to_string(), &envelop, from, content).await)
                        .is_ok()
                    {
                        break;
                    }
                }
            }

            if records.next().is_none() {
                log::error!(
                    target: vsmtp_config::log_channel::DELIVER,
                    "no valid mail exchanger found for '{}'",
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

/// filter recipients by domain name.
fn rcpt_by_domain(rcpt: &mut [Rcpt]) -> std::collections::HashMap<String, Vec<&mut Rcpt>> {
    rcpt.iter_mut()
        .fold(std::collections::HashMap::new(), |mut acc, rcpt| {
            if acc.contains_key(rcpt.address.domain()) {
                acc.get_mut(rcpt.address.domain()).unwrap().push(rcpt);
            } else {
                acc.insert(rcpt.address.domain().to_string(), vec![rcpt]);
            }

            acc
        })
}

/// fetch mx records for a specific domain.
async fn get_mx_records(
    resolver: &trust_dns_resolver::TokioAsyncResolver,
    query: &str,
) -> anyhow::Result<Vec<trust_dns_resolver::proto::rr::rdata::MX>> {
    let mut records_by_priority = resolver
        .mx_lookup(query)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    records_by_priority.sort_by_key(trust_dns_resolver::proto::rr::rdata::MX::preference);

    Ok(records_by_priority)
}

/// send an email using [lettre].
async fn send_email(
    config: &Config,
    target: &str,
    envelop: &lettre::address::Envelope,
    from: &vsmtp_common::address::Address,
    content: &str,
) -> anyhow::Result<()> {
    lettre::AsyncTransport::send_raw(
        // TODO: transport should be cached.
        &crate::transport::build_transport(config, from, target)?,
        envelop,
        content.as_bytes(),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod test {

    use vsmtp_common::address::Address;
    use vsmtp_config::{Config, ConfigServerDNS};

    use crate::transport::deliver::{get_mx_records, send_email};

    // use super::*;

    #[tokio::test]
    async fn test_get_mx_records() {
        // FIXME: find a way to guarantee that the mx records exists.
        let mut config = Config::default();
        config.server.dns = ConfigServerDNS::System;
        let dns = vsmtp_config::build_dns(&config).unwrap();

        get_mx_records(&dns, "google.com")
            .await
            .expect("couldn't find any mx records for google.com");

        assert!(get_mx_records(&dns, "invalid_query").await.is_err());
    }

    #[tokio::test]
    async fn test_delivery() {
        // NOTE: for this to return ok, we would need to setup a test server running locally.
        assert!(send_email(
            &Config::default(),
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
