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
// use anyhow::Context;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{anyhow, log},
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
    pub fn new(to: &ForwardTarget, resolver: &'r TokioAsyncResolver) -> Self {
        Self {
            to: to.clone(),
            resolver,
        }
    }
}

#[async_trait::async_trait]
impl<'r> Transport for Forward<'r> {
    async fn deliver(
        &mut self,
        config: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let envelop = super::build_lettre_envelop(from, to)
            .context("failed to build envelop to forward email")?;

        // if the domain is unknown, we ask the dns to get it (tls parameters required the domain).
        let target = match &self.to {
            ForwardTarget::Domain(domain) => domain.clone(),
            ForwardTarget::Ip(ip) => self
                .resolver
                .reverse_lookup(*ip)
                .await
                .context(format!("failed to forward email to {ip}"))?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("no domain found for {ip}"))?
                .to_string(),
        };

        match send_email(config, self.resolver, from, &target, &envelop, content).await {
            Ok(()) => {
                to.iter_mut()
                    .for_each(|rcpt| rcpt.email_status = EmailTransferStatus::Sent);
                return Ok(());
            }
            Err(err) => {
                log::debug!(
                    target: log_channels::FORWARD,
                    "(msg={}) failed to forward email to '{}': {err}",
                    metadata.message_id,
                    &target
                );

                for rcpt in to.iter_mut() {
                    rcpt.email_status = match rcpt.email_status {
                        EmailTransferStatus::HeldBack(count) => {
                            EmailTransferStatus::HeldBack(count)
                        }
                        _ => EmailTransferStatus::HeldBack(0),
                    };
                }

                anyhow::bail!("failed to forward email to '{}'", target)
            }
        }
    }
}

async fn send_email(
    config: &Config,
    resolver: &TokioAsyncResolver,
    from: &vsmtp_common::Address,
    target: &str,
    envelop: &lettre::address::Envelope,
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
