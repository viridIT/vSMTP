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
use anyhow::Context;
use vsmtp_common::mail_context::{Body, MailContext};
use vsmtp_config::server_config::ServerConfig;
use vsmtp_server::resolver::Resolver;

/// This delivery will send the mail to another MTA (relaying)
#[derive(Default)]
pub struct SMTPResolver;

#[async_trait::async_trait]
impl Resolver for SMTPResolver {
    // NOTE: should the function short circuit when sending an email failed ?
    async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
        let envelop = build_envelop(ctx).context("failed to build envelop to deliver email")?;
        let resolver = build_resolver().context("failed to build resolver to deliver email")?;
        let content = match &ctx.body {
            Body::Empty => anyhow::bail!("failed to send email: body is empty"),
            Body::Raw(raw) => raw.clone(),
            Body::Parsed(mail) => mail.raw_body(),
        };

        for rcpt in &ctx.envelop.rcpt {
            let query = rcpt.domain();
            let records = match get_mx_records(&resolver, query).await {
                Ok(records) => records,
                Err(err) => {
                    log::error!("failed to get mx records for '{}': {}", query, err);
                    continue;
                }
            };

            if records.iter().any(|record| {
                match send_email(&record.exchange().to_ascii(), &envelop, &content) {
                    Ok(_) => true,
                    Err(err) => {
                        log::debug!("failed to relay mail to {}: {}", rcpt, err);
                        false
                    }
                }
            }) {
                log::debug!("email successfully delivered to '{}'", rcpt);
            } else {
                log::error!("no valid mail exchanger found for '{}'", rcpt);
            }
        }
        Ok(())
    }
}

fn build_envelop(ctx: &MailContext) -> anyhow::Result<lettre::address::Envelope> {
    Ok(lettre::address::Envelope::new(
        Some(ctx.envelop.mail_from.full().parse()?),
        ctx.envelop
            .rcpt
            .iter()
            // NOTE: address that couldn't be converted will be silently dropped.
            .flat_map(|rcpt| rcpt.full().parse::<lettre::Address>())
            .collect(),
    )?)
}

fn build_resolver() -> anyhow::Result<trust_dns_resolver::TokioAsyncResolver> {
    Ok(trust_dns_resolver::TokioAsyncResolver::tokio(
        trust_dns_resolver::config::ResolverConfig::default(),
        trust_dns_resolver::config::ResolverOpts::default(),
    )?)
}

async fn get_mx_records(
    resolver: &trust_dns_resolver::TokioAsyncResolver,
    query: &str,
) -> anyhow::Result<Vec<trust_dns_resolver::proto::rr::rdata::MX>> {
    let mut mxs_by_priority = resolver
        .mx_lookup(query)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    mxs_by_priority.sort_by_key(trust_dns_resolver::proto::rr::rdata::MX::preference);

    Ok(mxs_by_priority)
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

#[cfg(test)]
mod test {

    use vsmtp_common::address::Address;

    use crate::resolver::get_default_context;

    use super::*;

    #[test]
    fn test_build_envelop() {
        let mut ctx = get_default_context();

        assert!(build_envelop(&ctx).is_err());

        ctx.envelop
            .rcpt
            .insert(Address::new("john@doe.com").unwrap());

        build_envelop(&ctx).expect("failed to build the envelop");
    }

    #[test]
    fn test_build_resolver() {
        // FIXME: find a way to make this function fail.
        assert!(build_resolver().is_ok());
    }

    #[tokio::test]
    async fn test_get_mx_records() {
        // FIXME: find a way to guarantee that the mx records exists.
        let resolver = build_resolver().expect("could not build resolver");

        get_mx_records(&resolver, "google.com")
            .await
            .expect("couldn't find any mx records for google.com");

        assert!(get_mx_records(&resolver, "invalid_query").await.is_err());
    }

    #[tokio::test]
    async fn test_delivery() {
        let mut ctx = get_default_context();
        ctx.envelop.mail_from = Address::new("john@doe.com").unwrap();
        ctx.envelop
            .rcpt
            .insert(Address::new("green@foo.com").unwrap());

        let envelop = build_envelop(&ctx).expect("failed to build envelop to deliver email");

        // NOTE: for this to return ok, we would need to setup a test server running locally.
        assert!(send_email("127.0.0.1", &envelop, "content").is_err());
    }
}
