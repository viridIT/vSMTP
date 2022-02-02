use crate::config::server_config::ServerConfig;
/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::model::mail::{Body, MailContext};

use anyhow::Context;
use lettre::{SmtpTransport, Transport};
use trust_dns_resolver::config::*;
use trust_dns_resolver::TokioAsyncResolver;

use super::Resolver;

#[derive(Default)]
pub struct SMTPResolver;

#[async_trait::async_trait]
impl Resolver for SMTPResolver {
    async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
        let envelop = lettre::address::Envelope::new(
            Some(ctx.envelop.mail_from.full().parse()?),
            ctx.envelop
                .rcpt
                .iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .flat_map(|rcpt| rcpt.full().parse::<lettre::Address>())
                .collect(),
        )?;

        let resolver =
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
                .context("failed to build resolver with trust-dns-resolver")?;

        for rcpt in ctx.envelop.rcpt.iter() {
            match resolver.mx_lookup(rcpt.domain()).await {
                Ok(mxs) => {
                    let mut mxs_by_priority = mxs.into_iter().collect::<Vec<_>>();
                    mxs_by_priority.sort_by_key(|mx| mx.preference());

                    for record in mxs_by_priority.iter() {
                        let exchange = record.exchange().to_ascii();

                        let tls_parameters = lettre::transport::smtp::client::TlsParameters::new(
                            exchange.as_str().into(),
                        )
                        .context("couldn't build tls parameters in smtp resolver")?;

                        let mailer = SmtpTransport::builder_dangerous(exchange.as_str())
                            .port(25)
                            .tls(lettre::transport::smtp::client::Tls::Required(
                                tls_parameters,
                            ))
                            .build();

                        let result = match &ctx.body {
                            Body::Parsed(mail) => {
                                mailer.send_raw(&envelop, mail.to_raw().1.as_bytes())
                            }
                            Body::Raw(raw) => mailer.send_raw(&envelop, raw.as_bytes()),
                        };

                        match result {
                            Ok(_) => {
                                log::debug!("email to {rcpt} sent successfully.");
                                break;
                            }
                            Err(err) => log::warn!("could not send email to {rcpt}: {err:?}, looking for other exchangers ..."),
                        };
                    }

                    log::error!("could not send email to any mail exchanger for {rcpt}, see warnings above.");
                }
                Err(err) => log::error!("could not send email to {rcpt}: {err}"),
            }
        }
        Ok(())
    }
}
