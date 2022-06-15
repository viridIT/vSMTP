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
use super::{get_mx_records, Transport};
use crate::transport::log_channels;
use anyhow::Context;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::{filter_by_domain_mut, Rcpt},
    re::{anyhow, lettre, log},
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
        from: &vsmtp_common::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        for (query, rcpt) in &mut filter_by_domain_mut(to) {
            let envelop = vsmtp_common::envelop::build_lettre(
                from,
                // TODO: 'to' parameter should be immutable, and the deliver
                //       implementor should return a new set of recipients.
                &rcpt.iter().map(|rcpt| (**rcpt).clone()).collect::<Vec<_>>()[..],
            )
            .context(format!(
                "failed to build envelop to deliver email for '{query}'"
            ))?;

            // getting mx records for a set of recipients.
            let records = match get_mx_records(self.resolver, query).await {
                Ok(records) => records,
                Err(err) => {
                    log::warn!(
                        target: log_channels::DELIVER,
                        "(msg={}) failed to get mx records for '{query}': {err}",
                        metadata.message_id
                    );

                    // could not find any mx records, we just skip all recipient in the group.
                    update_rcpt_held_back(rcpt);

                    continue;
                }
            };

            if records.is_empty() {
                log::warn!(
                    target: log_channels::DELIVER,
                    "(msg={}) empty set of MX records found for '{query}'",
                    metadata.message_id
                );

                // using directly the AAAA record instead of an mx record.
                // see https://www.rfc-editor.org/rfc/rfc5321#section-5.1
                match send_email(config, self.resolver, query, &envelop, from, content).await {
                    Ok(()) => update_rcpt_sent(rcpt),
                    Err(err) => {
                        update_rcpt_held_back(rcpt);

                        log::error!(
                            target: log_channels::DELIVER,
                            "(msg={}) failed to send message from '{from}' for '{query}': {err}",
                            metadata.message_id
                        );
                    }
                }
            } else {
                let mut records = records.iter();
                for record in records.by_ref() {
                    let host = record.exchange().to_ascii();

                    // checking for a null mx record.
                    // see https://datatracker.ietf.org/doc/html/rfc7505
                    if host == "." {
                        log::warn!(
                            target: log_channels::DELIVER,
                            "(msg={}) trying to delivery to '{query}', but a null mx record was found. '{query}' does not want to receive messages.",
                            metadata.message_id
                        );

                        update_rcpt_failed(rcpt, "null record found for this domain");

                        break;
                    }

                    match send_email(config, self.resolver, &host, &envelop, from, content).await {
                        // if a transfer succeeded, we can stop the lookup.
                        Ok(_) => break,
                        Err(err) => log::warn!(
                            target: log_channels::DELIVER,
                            "(msg={}) failed to send message from '{from}' for '{query}': {err}",
                            metadata.message_id
                        ),
                    }
                }

                if records.next().is_none() {
                    log::error!(
                        target: log_channels::DELIVER,
                        "(msg={}) no valid mail exchanger found for '{query}', check warnings above.",
                        metadata.message_id
                    );

                    update_rcpt_held_back(rcpt);
                } else {
                    update_rcpt_sent(rcpt);
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

fn update_rcpt_held_back(rcpt: &mut [&mut Rcpt]) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = match rcpt.email_status {
            EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count + 1),
            _ => EmailTransferStatus::HeldBack(0),
        };
    }
}

fn update_rcpt_sent(rcpt: &mut [&mut Rcpt]) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = EmailTransferStatus::Sent;
    }
}

fn update_rcpt_failed(rcpt: &mut [&mut Rcpt], reason: &str) {
    for rcpt in rcpt.iter_mut() {
        rcpt.email_status = EmailTransferStatus::Failed(reason.to_string());
    }
}

#[cfg(test)]
mod test {

    use super::update_rcpt_held_back;
    use crate::transport::deliver::{
        get_mx_records, send_email, update_rcpt_failed, update_rcpt_sent,
    };
    use trust_dns_resolver::TokioAsyncResolver;
    use vsmtp_common::{
        addr,
        rcpt::Rcpt,
        re::{lettre, tokio},
        transfer::EmailTransferStatus,
    };
    use vsmtp_config::{field::FieldServerDNS, Config};

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

    #[tokio::test]
    async fn test_get_mx_records() {
        // FIXME: find a way to guarantee that the mx records exists.
        let mut config = Config::default();
        config.server.dns = FieldServerDNS::System;
        let resolvers = vsmtp_config::build_resolvers(&config).unwrap();
        let dns = resolvers.get(&config.server.domain).unwrap();

        get_mx_records(dns, "google.com")
            .await
            .expect("couldn't find any mx records for google.com");

        assert!(get_mx_records(dns, "invalid_query").await.is_err());
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
