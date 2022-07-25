//! vSMTP server

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

#![doc(html_no_source)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::use_self)]

#[cfg(test)]
mod tests;

mod channel_message;
mod delivery;
mod processing;
mod receiver;
mod runtime;
mod server;

pub use receiver::MailHandler;

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{AbstractIO, Connection, OnMail};
pub use runtime::start_runtime;
pub use server::{socket_bind_anyhow, Server};

/// tag for a specific email process.
#[derive(Debug, vsmtp_common::re::strum::Display)]
pub enum Process {
    /// The server handle clients, parse commands & store emails at this stage.
    Receiver,
    /// The server handle emails "offline", the client is no longer communicating.
    Processing,
    /// The server is going to deliver the email locally or to another server.
    Delivery,
}

use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        lettre,
    },
    transfer::SmtpConnection,
    MessageBody,
};

/// delegate a message to another service.
pub(crate) fn delegate(
    delegator: &SmtpConnection,
    context: &MailContext,
    message: &MessageBody,
) -> anyhow::Result<lettre::transport::smtp::response::Response> {
    use lettre::Transport;

    let envelope = lettre::address::Envelope::new(
        Some(context.envelop.mail_from.full().parse()?),
        context
            .envelop
            .rcpt
            .iter()
            .map(|rcpt| {
                rcpt.address
                    .full()
                    .parse::<lettre::Address>()
                    .with_context(|| format!("failed to parse address {}", rcpt.address.full()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
    )?;

    delegator
        .0
        .lock()
        .unwrap()
        .send_raw(&envelope, message.inner().to_string().as_bytes())
        .context("failed to delegate email")
}
