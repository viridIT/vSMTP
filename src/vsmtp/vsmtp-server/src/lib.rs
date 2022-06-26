//! vSMTP server

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

mod log_channels {
    pub const SERVER: &str = "server::server";
    pub const AUTH: &str = "server::receiver::auth";
    pub const CONNECTION: &str = "server::receiver::connection";
    pub const TRANSACTION: &str = "server::receiver::transaction";
    pub const RUNTIME: &str = "server::runtime";
    pub const DEFERRED: &str = "server::processes::deferred";
    pub const DELIVERY: &str = "server::processes::delivery";
    pub const POSTQ: &str = "server::processes::postq";
    pub const PREQ: &str = "server::processes::preq";
}

mod channel_message;
mod delivery;
mod processing;
mod receiver;
mod runtime;
mod server;

use lettre::Transport;
pub use receiver::MailHandler;

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{handle_connection, AbstractIO, Connection, OnMail};
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
    mail_context::{MailContext, MessageBody},
    re::{
        anyhow::{self, Context},
        lettre,
    },
    transfer::SmtpConnection,
};

/// delegate a message to another service.
pub(crate) fn delegate(
    delegator: &SmtpConnection,
    context: &MailContext,
    message: &MessageBody,
) -> anyhow::Result<lettre::transport::smtp::response::Response> {
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
        .send_raw(&envelope, message.to_string().as_bytes())
        .context("failed to delegate email")
}
