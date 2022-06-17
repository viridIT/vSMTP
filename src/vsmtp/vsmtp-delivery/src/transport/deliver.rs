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

    async fn deliver_domain(
        &self,
        config: &Config,
        metadata: &MessageMetadata,
        content: &str,
        from: &Address,
        domain: &str,
        rcpt: Vec<lettre::Address>,
    ) -> anyhow::Result<ResultSendMail> {
        let envelop = lettre::address::Envelope::new(Some(from.full().parse()?), rcpt)?;

        // getting mx records for a set of recipients.
        let records = match self.get_mx_records(domain).await {
            Ok(records) => records,
            Err(err) => {
                log::warn!(
                    target: log_channels::DELIVER,
                    "(msg={}) failed to get mx records for '{domain}': {err}",
                    metadata.message_id
                );

                // could not find any mx records, we just skip all recipient in the group.
                // update_rcpt_held_back(&rcpt[..]);

                return Ok(ResultSendMail::IncreaseHeldBack);
            }
        };

        if records.is_empty() {
            log::warn!(
                target: log_channels::DELIVER,
                "(msg={}) empty set of MX records found for '{domain}'",
                metadata.message_id
            );

            // using directly the AAAA record instead of an mx record.
            // see https://www.rfc-editor.org/rfc/rfc5321#section-5.1
            match send_email(config, self.resolver, domain, &envelop, from, content).await {
                Ok(()) => return Ok(ResultSendMail::Sent), // update_rcpt_sent(&rcpt[..]),
                Err(err) => {
                    // update_rcpt_held_back(&rcpt[..]);

                    log::error!(
                        target: log_channels::DELIVER,
                        "(msg={}) failed to send message from '{from}' for '{domain}': {err}",
                        metadata.message_id
                    );

                    return Ok(ResultSendMail::IncreaseHeldBack);
                }
            }
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

                // update_rcpt_failed(&rcpt[..], );

                // break;
                return Ok(ResultSendMail::Failed(
                    "null record found for this domain".to_string(),
                ));
            }

            match send_email(config, self.resolver, &host, &envelop, from, content).await {
                // if a transfer succeeded, we can stop the lookup.
                Ok(_) => break,
                Err(err) => log::warn!(
                    target: log_channels::DELIVER,
                    "(msg={}) failed to send message from '{from}' for '{domain}': {err}",
                    metadata.message_id
                ),
            }
        }

        if records.next().is_none() {
            log::error!(
                target: log_channels::DELIVER,
                "(msg={}) no valid mail exchanger found for '{domain}', check warnings above.",
                metadata.message_id
            );

            // update_rcpt_held_back(&rcpt[..]);
            return Ok(ResultSendMail::IncreaseHeldBack);
        }

        // update_rcpt_sent(&rcpt[..]);
        Ok(ResultSendMail::Sent)
    }
}

#[async_trait::async_trait]
impl<'r> Transport for Deliver<'r> {
    async fn deliver(
        &mut self,
        config: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::Address,
        to: Vec<Rcpt>,
        content: &str,
    ) -> Vec<Rcpt> {
        let mut acc = std::collections::HashMap::<String, Vec<Rcpt>>::new();
        for rcpt in to {
            if let Some(domain) = acc.get_mut(rcpt.address.domain()) {
                domain.push(rcpt);
            } else {
                acc.insert(rcpt.address.domain().to_string(), vec![rcpt]);
            }
        }

        for (domain, rcpt) in &mut acc {
            let updated_status = self
                .deliver_domain(
                    config,
                    metadata,
                    content,
                    from,
                    domain,
                    rcpt.iter()
                        .map(|i| {
                            i.address
                                .full()
                                .parse::<lettre::Address>()
                                .context("failed to parse address")
                        })
                        .collect::<anyhow::Result<Vec<_>>>()
                        .unwrap(),
                )
                .await
                .unwrap();

            match updated_status {
                ResultSendMail::IncreaseHeldBack => rcpt.iter_mut().for_each(|i| {
                    i.email_status = EmailTransferStatus::HeldBack(match i.email_status {
                        EmailTransferStatus::HeldBack(count) => count + 1,
                        _ => 0,
                    });
                }),
                ResultSendMail::Sent => rcpt.iter_mut().for_each(|i| {
                    i.email_status = EmailTransferStatus::Sent;
                }),
                ResultSendMail::Failed(reason) => {
                    for i in rcpt {
                        i.email_status = EmailTransferStatus::Failed(reason.clone());
                    }
                }
            };
        }

        acc.into_iter().fold(vec![], |mut acc, (_, rcpt)| {
            acc.extend(rcpt);
            acc
        })
    }
}

/// send an email using [lettre].
async fn send_email(
    config: &Config,
    resolver: &TokioAsyncResolver,
    target: &str,
    envelop: &lettre::address::Envelope,
    from: &vsmtp_common::Address,
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

enum ResultSendMail {
    IncreaseHeldBack,
    Sent,
    Failed(String),
}

/*
fn update_rcpt_held_back(rcpt: &[&mut Rcpt]) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = match rcpt.email_status {
            EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count + 1),
            _ => EmailTransferStatus::HeldBack(0),
        };
    }
}

fn update_rcpt_sent(rcpt: &[&mut Rcpt]) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = EmailTransferStatus::Sent;
    }
}

fn update_rcpt_failed(rcpt: &[&mut Rcpt], reason: &str) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = EmailTransferStatus::Failed(reason.to_string());
    }
}
*/

#[cfg(test)]
mod test {

    use crate::transport::deliver::{send_email, Deliver};
    use trust_dns_resolver::TokioAsyncResolver;
    use vsmtp_common::{
        addr,
        re::{lettre, tokio},
    };
    use vsmtp_config::{field::FieldServerDNS, Config};

    /*
    #[test]
    fn test_update_rcpt_held_back() {
        let mut rcpt1 = Rcpt::new(addr!("john.doe@example.com"));
        let mut rcpt2 = Rcpt::new(addr!("green.foo@example.com"));
        let mut rcpt3 = Rcpt::new(addr!("bar@example.com"));
        let mut rcpt = vec![&mut rcpt1, &mut rcpt2, &mut rcpt3];

        update_rcpt_held_back(&mut rcpt[..]);

        assert!(rcpt
            .iter()
            .all(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::HeldBack(_))));

        update_rcpt_sent(&mut rcpt[..]);

        assert!(rcpt
            .iter()
            .all(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::Sent)));

        update_rcpt_failed(&mut rcpt[..], "could not send email to this domain");

        assert!(rcpt
            .iter()
            .all(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::Failed(_))));
    }
    */

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
            &addr!("a@a.a"),
            "content"
        )
        .await
        .is_err());
    }
}
