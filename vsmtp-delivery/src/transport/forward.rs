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
use vsmtp_common::{mail_context::MessageMetadata, rcpt::Rcpt, transfer::EmailTransferStatus};
use vsmtp_config::Config;

/// the email will be directly delivered to the server, without mx lookup.
#[derive(Default)]
pub struct Forward(pub String);

#[async_trait::async_trait]
impl Transport for Forward {
    async fn deliver(
        &mut self,
        _: &Config,
        _: &TokioAsyncResolver,
        _: &MessageMetadata,
        from: &vsmtp_common::address::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let envelop = super::build_lettre_envelop(from, &to[..])
            .context("failed to build envelop to forward email")?;

        for rcpt in to {
            match send_email(rcpt.address.domain(), &envelop, content) {
                Ok(()) => rcpt.email_status = EmailTransferStatus::Sent,
                Err(err) => {
                    log::error!(
                        target: vsmtp_config::log_channel::DELIVER,
                        "no valid mail exchanger found for '{}': {}",
                        rcpt.address.domain(),
                        err
                    );

                    rcpt.email_status = match rcpt.email_status {
                        EmailTransferStatus::HeldBack(count) => {
                            EmailTransferStatus::HeldBack(count)
                        }
                        _ => EmailTransferStatus::HeldBack(0),
                    };
                }
            }
        }

        Ok(())
    }
}

fn send_email(
    exchange: &str,
    envelop: &lettre::address::Envelope,
    content: &str,
) -> anyhow::Result<()> {
    let tls_parameters = lettre::transport::smtp::client::TlsParameters::new(exchange.into())?;

    let mailer = lettre::SmtpTransport::builder_dangerous(exchange)
        .port(25)
        .tls(lettre::transport::smtp::client::Tls::Required(
            tls_parameters,
        ))
        .build();

    lettre::Transport::send_raw(&mailer, envelop, content.as_bytes())?;
    Ok(())
}
