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
#![allow(clippy::doc_markdown)]

#[cfg(test)]
mod tests;

mod channel_message;
mod queue;
mod receiver;
mod runtime;
mod server;
mod processes {
    pub mod delivery;
    pub mod mime;
}

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{handle_connection, Connection, ConnectionKind, IoService, OnMail};
pub use runtime::start_runtime;
pub use server::Server;

/// re-exported module
pub mod re {
    pub use base64;
    pub use tokio;
}
