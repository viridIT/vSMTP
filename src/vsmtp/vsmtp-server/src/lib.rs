//! vSMTP server

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]
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
mod receiver;
mod runtime;
mod server;
mod processes {
    pub mod delivery;
    pub mod postq;
}

pub use receiver::MailHandler;

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{handle_connection, AbstractIO, Connection, ConnectionKind, OnMail};
pub use runtime::start_runtime;
pub use server::{socket_bind_anyhow, Server};

/// re-exported module
pub mod re {
    pub use tokio;
}
