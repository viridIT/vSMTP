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

use lettre::{Message, SmtpTransport, Transport};
use trust_dns_resolver::config::*;
use trust_dns_resolver::TokioAsyncResolver;

#[derive(Default)]
pub struct SMTPResolver;

impl SMTPResolver {
    pub async fn deliver(&self, ctx: &MailContext) -> Result<(), std::io::Error> {
        if let Body::Parsed(mail) = &ctx.body {
            let mut builder = Message::builder();
            for header in mail.headers.iter() {
                builder = match (header.0.as_str(), header.1.as_str()) {
                    ("from", value) => builder.from(value.parse().unwrap()),
                    ("to", value) => {
                        for inbox in value.split(", ") {
                            builder = builder.to(inbox.parse().unwrap())
                        }
                        builder
                    }
                    ("subject", value) => builder.subject(value),
                    _ => builder,
                };
            }

            let to_send = builder
                .body(mail.to_raw().1)
                .expect("failed to build email with lettre");
            let resolver =
                TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
                    .expect("failed to build resolver with trust-dns-resolver");

            for rcpt in ctx.envelop.rcpt.iter() {
                let domain = rcpt.domain();
                let mx = resolver.mx_lookup(domain).await;

                match mx {
                    Err(err) => log::error!("could not send email to {rcpt}: {err}"),
                    Ok(mxs) => {
                        let mut mxs_by_priority = mxs.into_iter().collect::<Vec<_>>();
                        mxs_by_priority.sort_by_key(|mx| mx.preference());

                        for record in mxs_by_priority.iter() {
                            let exchange = record.exchange().to_ascii();

                            let tls_parameters =
                                lettre::transport::smtp::client::TlsParameters::new(
                                    exchange.as_str().into(),
                                )
                                .expect("couldn't build tls parameters");

                            let mailer = SmtpTransport::builder_dangerous(exchange.as_str())
                                .port(25)
                                .tls(lettre::transport::smtp::client::Tls::Required(
                                    tls_parameters,
                                ))
                                .build();

                            match mailer.send(&to_send) {
                                Ok(_) => {
                                    log::debug!("email to {rcpt} sent successfully.");
                                    break;
                                }
                                Err(err) => log::warn!("could not send email to {rcpt}: {err:?}, looking for other exchangers ..."),
                            };
                        }
                    }
                }
            }
        } else {
            log::error!("email hasn't been parsed, exiting delivery ...");
        }

        Ok(())
    }
}
