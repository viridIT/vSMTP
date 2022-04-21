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

mod channel_message;
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
    pub use tokio;
}
