//! vSMTP delivery system

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

/*
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
*/

/// a few helpers to create systems that will deliver emails.
pub mod transport {
    use vsmtp_common::{address::Address, mail_context::MessageMetadata, rcpt::Rcpt};
    use vsmtp_config::Config;

    /// allowing the [ServerVSMTP] to deliver a mail.
    #[async_trait::async_trait]
    pub trait Transport {
        /// the deliver method of the [Resolver] trait
        async fn deliver(
            &mut self,
            config: &Config,
            metadata: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            content: &str,
        ) -> anyhow::Result<()>;
    }

    pub(super) mod deliver;
    pub(super) mod forward;
    pub(super) mod maildir;
    pub(super) mod mbox;

    /// no transfer will be made if this resolver is selected.
    pub(super) struct NoTransfer;

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

    /// build a [lettre] envelop using from address & recipients.
    fn build_lettre_envelop(
        from: &vsmtp_common::address::Address,
        rcpt: &[Rcpt],
    ) -> anyhow::Result<lettre::address::Envelope> {
        Ok(lettre::address::Envelope::new(
            Some(from.full().parse()?),
            rcpt.iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .flat_map(|rcpt| rcpt.address.full().parse::<lettre::Address>())
                .collect(),
        )?)
    }

    /// create a list of transports identified by their Transfer key metadata.
    #[must_use]
    pub fn create_transports(
    ) -> std::collections::HashMap<vsmtp_common::transfer::Transfer, Box<dyn Transport + Send + Sync>>
    {
        let mut resolvers = std::collections::HashMap::<
            vsmtp_common::transfer::Transfer,
            Box<dyn Transport + Send + Sync>,
        >::new();
        resolvers.insert(
            vsmtp_common::transfer::Transfer::Forward,
            Box::new(forward::Forward::default()),
        );
        resolvers.insert(
            vsmtp_common::transfer::Transfer::Deliver,
            Box::new(deliver::Deliver::default()),
        );
        resolvers.insert(
            vsmtp_common::transfer::Transfer::Maildir,
            Box::new(maildir::MailDir::default()),
        );
        resolvers.insert(
            vsmtp_common::transfer::Transfer::Mbox,
            Box::new(mbox::MBox::default()),
        );
        resolvers.insert(
            vsmtp_common::transfer::Transfer::None,
            Box::new(NoTransfer {}),
        );
        resolvers
    }
}

#[cfg(test)]
pub mod test {
    #[must_use]
    /// create an empty email context for testing purposes.
    pub fn get_default_context() -> vsmtp_common::mail_context::MailContext {
        vsmtp_common::mail_context::MailContext {
            body: vsmtp_common::mail_context::Body::Empty,
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop::default(),
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        }
    }
}
