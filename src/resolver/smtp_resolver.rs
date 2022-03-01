use super::Resolver;
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
use crate::config::server_config::ServerConfig;
use crate::smtp::mail::Body;
use crate::smtp::mail::MailContext;

/// This delivery will send the mail to another MTA (relaying)
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

        let resolver = anyhow::Context::context(
            trust_dns_resolver::TokioAsyncResolver::tokio(
                trust_dns_resolver::config::ResolverConfig::default(),
                trust_dns_resolver::config::ResolverOpts::default(),
            ),
            "failed to build resolver with trust-dns-resolver",
        )?;

        for rcpt in ctx.envelop.rcpt.iter() {
            match resolver.mx_lookup(rcpt.domain()).await {
                Ok(mxs) => {
                    let mut mxs_by_priority = mxs.into_iter().collect::<Vec<_>>();
                    mxs_by_priority.sort_by_key(|mx| mx.preference());

                    for record in mxs_by_priority.iter() {
                        let exchange = record.exchange().to_ascii();

                        let tls_parameters = anyhow::Context::context(
                            lettre::transport::smtp::client::TlsParameters::new(
                                exchange.as_str().into(),
                            ),
                            "couldn't build tls parameters in smtp resolver",
                        )?;

                        let mailer = lettre::SmtpTransport::builder_dangerous(exchange.as_str())
                            .port(25)
                            .tls(lettre::transport::smtp::client::Tls::Required(
                                tls_parameters,
                            ))
                            .build();

                        let result = match &ctx.body {
                            Body::Empty => anyhow::bail!("failed to send email: body is empty"),
                            Body::Raw(raw) => {
                                lettre::Transport::send_raw(&mailer, &envelop, raw.as_bytes())
                            }
                            Body::Parsed(mail) => lettre::Transport::send_raw(
                                &mailer,
                                &envelop,
                                mail.to_raw().1.as_bytes(),
                            ),
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
