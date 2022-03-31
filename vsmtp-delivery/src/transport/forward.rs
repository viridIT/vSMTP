/**
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
**/
use super::Transport;

use anyhow::Context;
use trust_dns_resolver::TokioAsyncResolver;
// use anyhow::Context;
use vsmtp_common::{
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{anyhow, log},
    transfer::EmailTransferStatus,
};
use vsmtp_config::Config;

/// the email will be directly delivered to the server, without mx lookup.
#[derive(Default)]
pub struct Forward(pub String);

#[async_trait::async_trait]
impl Transport for Forward {
    async fn deliver(
        &mut self,
        config: &Config,
        dns: &TokioAsyncResolver,
        _: &MessageMetadata,
        from: &vsmtp_common::address::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let envelop = super::build_lettre_envelop(from, &to[..])
            .context("failed to build envelop to forward email")?;

        for destination in dns.lookup_ip(&self.0).await?.iter() {
            match send_email(config, from, &destination.to_string(), &envelop, content).await {
                Ok(()) => {
                    to.iter_mut()
                        .for_each(|rcpt| rcpt.email_status = EmailTransferStatus::Sent);
                    return Ok(());
                }
                Err(err) => {
                    log::debug!(
                        target: vsmtp_config::log_channel::DELIVER,
                        "failed to forward email to '{}': {}",
                        destination,
                        err
                    );
                }
            }
        }

        for rcpt in to.iter_mut() {
            rcpt.email_status = match rcpt.email_status {
                EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count),
                _ => EmailTransferStatus::HeldBack(0),
            };
        }

        anyhow::bail!("failed to forward email to '{}'", self.0)
    }
}

async fn send_email(
    config: &Config,
    from: &vsmtp_common::address::Address,
    target: &str,
    envelop: &lettre::address::Envelope,
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
