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
pub use crate::{config::server_config::ServerConfig, smtp::mail::MailContext};

/// Protocol Maildir
pub mod maildir_resolver;

/// Protocol Mailbox
pub mod mbox_resolver;

/// Mail relaying
pub mod smtp_resolver;

/// A trait allowing the [ServerVSMTP] to deliver a mail
#[async_trait::async_trait]
pub trait Resolver {
    /// the deliver method of the [Resolver] trait
    async fn deliver(&mut self, config: &ServerConfig, mail: &MailContext) -> anyhow::Result<()>;
}

#[cfg(test)]
fn get_default_context() -> MailContext {
    MailContext {
        body: crate::Body::Empty,
        connexion_timestamp: std::time::SystemTime::now(),
        client_addr: std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            0,
        ),
        envelop: Default::default(),
        metadata: Some(crate::smtp::mail::MessageMetadata {
            timestamp: std::time::SystemTime::now(),
            ..Default::default()
        }),
    }
}
