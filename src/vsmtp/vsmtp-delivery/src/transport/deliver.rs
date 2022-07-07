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
use super::Transport;
use crate::transport::log_channels;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{
        anyhow::{self, Context},
        lettre, log,
    },
    transfer::EmailTransferStatus,
    Address,
};
use vsmtp_config::Config;

enum ResultSendMail {
    /// Temporary error, increasing the `HeldBack` property to retry later.
    IncreaseHeldBack(anyhow::Error),
    /// Definitive error. Failed to send the mail.
    Failed(String),
}

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

impl<'r> Deliver<'r> {
    /// fetch mx records for a specific domain and order them by priority.
    async fn get_mx_records(
        &self,
        query: &str,
    ) -> anyhow::Result<Vec<trust_dns_resolver::proto::rr::rdata::MX>> {
        let mut records_by_priority = self
            .resolver
            .mx_lookup(query)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        records_by_priority.sort_by_key(trust_dns_resolver::proto::rr::rdata::MX::preference);

        Ok(records_by_priority)
    }

    async fn send_email(
        &self,
        config: &Config,
        target: &str,
        envelop: &lettre::address::Envelope,
        from: &vsmtp_common::Address,
        content: &str,
    ) -> anyhow::Result<()> {
        lettre::AsyncTransport::send_raw(
            // TODO: transport should be cached.
            &crate::transport::build_transport(config, self.resolver, from, target)?,
            envelop,
            content.as_bytes(),
        )
        .await?;

        Ok(())
    }

    // FIXME: should just return a `ResultSendMail`
    async fn deliver_one_domain(
        &self,
        config: &Config,
        metadata: &MessageMetadata,
        content: &str,
        from: &Address,
        domain: &str,
        rcpt: &[Rcpt],
    ) -> anyhow::Result<(), ResultSendMail> {
        let envelop = from
            .full()
            .parse()
            .context("envelop is invalid")
            .and_then(|from| {
                Ok((
                    from,
                    rcpt.iter()
                        .map(|i| {
                            i.address
                                .full()
                                .parse::<lettre::Address>()
                                .with_context(|| format!("receiver address is not valid: {i}"))
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?,
                ))
            })
            .and_then(|(from, rcpt_addresses)| {
                lettre::address::Envelope::new(Some(from), rcpt_addresses)
                    .context("envelop is invalid")
            })
            .map_err(|err| ResultSendMail::Failed(err.to_string()))?;

        let records = self
            .get_mx_records(domain)
            .await
            .with_context(|| {
                format!(
                    "(msg={}) failed to get mx records for '{domain}'",
                    metadata.message_id
                )
            })
            .map_err(ResultSendMail::IncreaseHeldBack)?;

        if records.is_empty() {
            log::warn!(
                target: log_channels::DELIVER,
                "(msg={}) empty set of MX records found for '{domain}'",
                metadata.message_id
            );

            // using directly the AAAA record instead of an mx record.
            // see https://www.rfc-editor.org/rfc/rfc5321#section-5.1
            self.send_email(config, domain, &envelop, from, content)
                .await
                .with_context(|| {
                    format!(
                        "(msg={}) failed to send message from '{from}' for '{domain}'",
                        metadata.message_id
                    )
                })
                .map_err(ResultSendMail::IncreaseHeldBack)?;
        }

        let mut records = records.iter();
        for record in records.by_ref() {
            let host = record.exchange().to_ascii();

            // checking for a null mx record.
            // see https://datatracker.ietf.org/doc/html/rfc7505
            if host == "." {
                log::warn!(
                    target: log_channels::DELIVER,
                    "(msg={}) trying to delivery to '{domain}', but a null mx record was found. '{domain}' does not want to receive messages.",
                    metadata.message_id
                );

                return Err(ResultSendMail::Failed(
                    "null record found for this domain".to_string(),
                ));
            }

            match self
                .send_email(config, &host, &envelop, from, content)
                .await
            {
                Ok(_) => return Ok(()),
                Err(err) => log::warn!(
                    target: log_channels::DELIVER,
                    "(msg={}) failed to send message from '{from}' for '{domain}': {err}",
                    metadata.message_id
                ),
            }
        }

        return Err(ResultSendMail::IncreaseHeldBack(anyhow::anyhow!(
            "no valid mail exchanger found for '{domain}'",
        )));
    }
}

#[async_trait::async_trait]
impl<'r> Transport for Deliver<'r> {
    async fn deliver(
        self,
        config: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::Address,
        to: Vec<Rcpt>,
        content: &str,
    ) -> Vec<Rcpt> {
        let mut rcpt_by_domain = std::collections::HashMap::<String, Vec<Rcpt>>::new();
        for rcpt in to {
            rcpt_by_domain
                .entry(rcpt.address.domain().to_string())
                .and_modify(|domain| domain.push(rcpt.clone()))
                .or_insert_with(|| vec![rcpt.clone()]);
        }

        for (domain, rcpt) in &mut rcpt_by_domain {
            // TODO: run the delivery on different domain concurrently

            match self
                .deliver_one_domain(config, metadata, content, from, domain, &*rcpt)
                .await
            {
                Ok(_) => rcpt.iter_mut().for_each(|i| {
                    i.email_status = EmailTransferStatus::Sent {
                        timestamp: std::time::SystemTime::now(),
                    };
                }),
                Err(ResultSendMail::IncreaseHeldBack(error)) => {
                    log::error!(
                        target: log_channels::DELIVER,
                        "(msg={}) TEMP ERROR, failed to send message from '{from}' for '{domain}': {error}",
                        metadata.message_id,
                        from = from.full(),
                        domain = domain,
                        error = error
                    );
                    for i in rcpt {
                        i.email_status.held_back(error.to_string());
                    }
                }
                Err(ResultSendMail::Failed(reason)) => {
                    log::error!(
                        target: log_channels::DELIVER,
                        "(msg={}) PERM ERROR, failed to send message from '{from}' for '{domain}': {reason}",
                        metadata.message_id,
                        from = from.full(),
                        domain = domain,
                        reason = reason
                    );

                    for i in rcpt {
                        i.email_status = EmailTransferStatus::Failed {
                            timestamp: std::time::SystemTime::now(),
                            reason: reason.clone(),
                        }
                    }
                }
            }
        }

        rcpt_by_domain
            .into_iter()
            .fold(vec![], |mut acc, (_, rcpt)| {
                acc.extend(rcpt);
                acc
            })
    }
}

#[cfg(test)]
mod test {

    use crate::transport::deliver::Deliver;
    use trust_dns_resolver::TokioAsyncResolver;
    use vsmtp_common::{
        addr,
        re::{lettre, tokio},
    };
    use vsmtp_config::{field::FieldServerDNS, Config};

    #[tokio::test]
    async fn test_get_mx_records() {
        // FIXME: find a way to guarantee that the mx records exists.
        let mut config = Config::default();
        config.server.dns = FieldServerDNS::System;
        let resolvers = vsmtp_config::build_resolvers(&config).unwrap();
        let deliver = Deliver::new(resolvers.get(&config.server.domain).unwrap());

        deliver
            .get_mx_records("google.com")
            .await
            .expect("couldn't find any mx records for google.com");

        assert!(deliver.get_mx_records("invalid_query").await.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_delivery() {
        let config = Config::default();

        let resolver = TokioAsyncResolver::tokio_from_system_conf().unwrap();
        let deliver = Deliver::new(&resolver);

        // NOTE: for this to return ok, we would need to setup a test server running locally.
        assert!(deliver
            .send_email(
                &config,
                "localhost",
                &lettre::address::Envelope::new(
                    Some("a@a.a".parse().unwrap()),
                    vec!["b@b.b".parse().unwrap()]
                )
                .unwrap(),
                &addr!("a@a.a"),
                "content"
            )
            .await
            .is_err());
    }
}
