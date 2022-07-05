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
}

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
pub use receiver::{handle_connection, AbstractIO, Connection, OnMail};
pub use runtime::start_runtime;
pub use server::{socket_bind_anyhow, Server};
