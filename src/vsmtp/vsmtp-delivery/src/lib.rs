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

//! vSMTP delivery system

#![doc(html_no_source)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

/// a few helpers to create systems that will deliver emails.
pub mod transport {
    use trust_dns_resolver::TokioAsyncResolver;
    use vsmtp_common::re::anyhow::Context;
    use vsmtp_common::re::lettre;
    use vsmtp_common::{mail_context::MessageMetadata, rcpt::Rcpt, re::anyhow, Address};
    use vsmtp_config::Config;

    mod log_channels {
        pub const DELIVER: &str = "server::delivery::deliver";
        pub const FORWARD: &str = "server::delivery::forward";
        pub const MAILDIR: &str = "server::delivery::maildir";
        pub const MBOX: &str = "server::delivery::mbox";
    }

    ///
    #[async_trait::async_trait]
    pub trait Transport {
        ///
        async fn deliver(
            &mut self,
            config: &Config,
            metadata: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            content: &str,
        ) -> anyhow::Result<()>;
    }

    /// delivery transport.
    pub mod deliver;
    /// forwarding transport.
    pub mod forward;
    /// maildir transport.
    pub mod maildir;
    /// mbox transport.
    pub mod mbox;

    /// no transfer will be made if this resolver is selected.
    pub struct NoTransfer;

    #[async_trait::async_trait]
    impl Transport for NoTransfer {
        async fn deliver(
            &mut self,
            _: &Config,
            _: &MessageMetadata,
            _: &Address,
            _: &mut [Rcpt],
            _: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    /// build a transport using opportunistic tls and toml specified certificates.
    /// TODO: resulting transport should be cached.
    fn build_transport(
        config: &Config,
        // will be used for tlsa record resolving.
        _: &TokioAsyncResolver,
        from: &vsmtp_common::Address,
        target: &str,
    ) -> anyhow::Result<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>> {
        let tls_builder =
            lettre::transport::smtp::client::TlsParameters::builder(target.to_string());

        // from's domain could match the root domain of the server.
        let tls_parameters =
            if config.server.domain == from.domain() && config.server.tls.is_some() {
                tls_builder.add_root_certificate(
                    lettre::transport::smtp::client::Certificate::from_der(
                        config
                            .server
                            .tls
                            .as_ref()
                            .unwrap()
                            .certificate
                            .inner
                            .0
                            .clone(),
                    )
                    .context("failed to parse certificate as der")?,
                )
            }
            // or a domain from one of the virtual domains.
            else if let Some(tls_config) = config
                .server
                .r#virtual
                .get(from.domain())
                .and_then(|domain| domain.tls.as_ref())
            {
                tls_builder.add_root_certificate(
                    lettre::transport::smtp::client::Certificate::from_der(
                        tls_config.certificate.inner.0.clone(),
                    )
                    .context("failed to parse certificate as der")?,
                )
            // if not, no certificate are used.
            } else {
                tls_builder
            }
            .build_rustls()
            .context("failed to build tls parameters")?;

        Ok(
            lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(target)
                .hello_name(lettre::transport::smtp::extension::ClientId::Domain(
                    from.domain().to_string(),
                ))
                .port(lettre::transport::smtp::SMTP_PORT)
                .tls(lettre::transport::smtp::client::Tls::Opportunistic(
                    tls_parameters,
                ))
                .build(),
        )
    }

    /// fetch mx records for a specific domain and order them by priority.
    async fn get_mx_records(
        resolver: &trust_dns_resolver::TokioAsyncResolver,
        query: &str,
    ) -> anyhow::Result<Vec<trust_dns_resolver::proto::rr::rdata::MX>> {
        let mut records_by_priority = resolver
            .mx_lookup(query)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        records_by_priority.sort_by_key(trust_dns_resolver::proto::rr::rdata::MX::preference);

        Ok(records_by_priority)
    }
}

#[cfg(test)]
pub mod test {
    /// create an empty email context for testing purposes.
    ///
    /// # Panics
    #[must_use]
    pub fn get_default_context() -> vsmtp_common::mail_context::MailContext {
        vsmtp_common::mail_context::MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
                server_address: "127.0.0.1:25".parse().unwrap(),
            },
            client_addr: "127.0.0.1:26".parse().unwrap(),
            envelop: vsmtp_common::envelop::Envelop::default(),
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        }
    }

    use vsmtp_common::{
        addr,
        envelop::build_lettre,
        mail_context::ConnectionContext,
        rcpt::Rcpt,
        re::lettre,
        transfer::{EmailTransferStatus, Transfer},
    };

    #[test]
    fn test_build_lettre_envelop() {
        assert_eq!(
            build_lettre(
                &addr!("a@a.a"),
                &[Rcpt {
                    address: addr!("b@b.b"),
                    transfer_method: Transfer::None,
                    email_status: EmailTransferStatus::Sent
                }]
            )
            .expect("failed to build lettre envelop"),
            lettre::address::Envelope::new(
                Some("a@a.a".parse().unwrap()),
                vec!["b@b.b".parse().unwrap()]
            )
            .unwrap()
        );
    }
}
