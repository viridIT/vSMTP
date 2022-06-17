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
use anyhow::Context;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{anyhow, lettre, log},
    transfer::{EmailTransferStatus, ForwardTarget},
};
use vsmtp_config::Config;

/// the email will be directly delivered to the server, without mx lookup.
pub struct Forward<'r> {
    to: ForwardTarget,
    resolver: &'r TokioAsyncResolver,
}

impl<'r> Forward<'r> {
    /// create a new deliver with a resolver to get data from the distant dns server.
    #[must_use]
    pub const fn new(to: ForwardTarget, resolver: &'r TokioAsyncResolver) -> Self {
        Self { to, resolver }
    }
}

impl<'r> Forward<'r> {
    async fn reverse_lookup(&self, query: &std::net::IpAddr) -> anyhow::Result<String> {
        let result = self
            .resolver
            .reverse_lookup(*query)
            .await
            .with_context(|| format!("failed to forward email to {query}"))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no domain found for {query}"))?
            .to_string();

        Ok(result)
    }

    async fn send_email(
        &self,
        config: &Config,
        from: &vsmtp_common::Address,
        target: &str,
        envelop: &lettre::address::Envelope,
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
}

#[async_trait::async_trait]
impl<'r> Transport for Forward<'r> {
    async fn deliver(
        &mut self,
        config: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::Address,
        to: Vec<Rcpt>,
        content: &str,
    ) -> Vec<Rcpt> {
        let envelop = lettre::address::Envelope::new(
            Some(from.full().parse().unwrap()),
            to.iter()
                .map(|rcpt| {
                    rcpt.address
                        .full()
                        .parse::<lettre::Address>()
                        .context("failed to parse address")
                })
                .collect::<anyhow::Result<Vec<_>>>()
                .unwrap(),
        )
        .unwrap();

        // if the domain is unknown, we ask the dns to get it (tls parameters required the domain).
        let target = match &self.to {
            ForwardTarget::Domain(domain) => domain.clone(),
            ForwardTarget::Ip(ip) => self.reverse_lookup(ip).await.unwrap(),
            ForwardTarget::Socket(socket) => self.reverse_lookup(&socket.ip()).await.unwrap(),
        };

        match self
            .send_email(config, from, &target, &envelop, content)
            .await
        {
            Ok(()) => {
                return to
                    .into_iter()
                    .map(|rcpt| Rcpt {
                        email_status: EmailTransferStatus::Sent,
                        ..rcpt
                    })
                    .collect();
            }
            Err(err) => {
                log::debug!(
                    target: log_channels::FORWARD,
                    "(msg={}) failed to forward email to '{}': {err}",
                    metadata.message_id,
                    &target
                );

                return to
                    .into_iter()
                    .map(|rcpt| Rcpt {
                        email_status: EmailTransferStatus::HeldBack(match rcpt.email_status {
                            EmailTransferStatus::HeldBack(count) => count + 1,
                            _ => 0,
                        }),
                        ..rcpt
                    })
                    .collect();
            }
        }
    }
}
