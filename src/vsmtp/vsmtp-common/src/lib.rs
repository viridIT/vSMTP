//! vSMTP common definition

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
#![allow(clippy::use_self)] // false positive with enums

/// Default smtp port
pub const SMTP_PORT: u16 = 25;

/// Default submission port
pub const SUBMISSION_PORT: u16 = 587;

/// Default submission over TLS port
///
/// Defined in [RFC8314](https://tools.ietf.org/html/rfc8314)
pub const SUBMISSIONS_PORT: u16 = 465;

/// Type of SMTP connection.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, strum::Display)]
pub enum ConnectionKind {
    /// Connection coming for relay (MTA on port 25)
    /// see <https://datatracker.ietf.org/doc/html/rfc5321>
    Relay,
    /// Connection coming for submission (MSA on port 587)
    /// see <https://datatracker.ietf.org/doc/html/rfc6409>
    Submission,
    /// Connection coming for submissionS (MSA on port 465)
    /// see <https://datatracker.ietf.org/doc/html/rfc8314>
    Tunneled,
}

mod log_channels {
    pub const QUEUE: &str = "server::queue";
}

#[macro_use]
mod r#type {
    #[macro_use]
    pub mod address;
    pub mod code_id;
    pub mod reply;
    pub mod reply_code;
}

mod either;
pub use either::Either;

mod message {
    pub mod mail;
    #[allow(clippy::module_name_repetitions)]
    pub mod message_body;
    pub mod mime_type;
}

pub use message::{
    mail::*,
    message_body::{MessageBody, RawBody},
    mime_type::*,
};
pub use r#type::{address::Address, code_id::CodeID, reply::Reply, reply_code::*};

///
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum ReplyOrCodeID {
    ///
    CodeID(CodeID),
    ///
    Reply(Reply),
}

/// envelop of a transaction
pub mod envelop;

/// parsed command of the client
pub mod event;

/// abstraction of the libc
pub mod libc_abstraction;

/// content generated by a smtp transaction
pub mod mail_context;

/// state of a smtp transaction
pub mod state;

/// status of the mail context
pub mod status;

/// rcpt data structure.
pub mod rcpt;

/// queues
pub mod queue;

/// transfer method for delivery / forwarding.
pub mod transfer;

/// parsing utils.
pub mod utils;

/// Data related to ESMTP Authentication
pub mod auth {
    mod credentials;
    mod mechanism;

    pub use credentials::Credentials;
    pub use mechanism::Mechanism;
}

mod r#trait {
    pub mod mail_parser;
}

pub use r#trait::mail_parser::{MailParser, MailParserOnFly, ParserOutcome};

#[cfg(test)]
mod tests {
    mod event;

    mod libc_abstraction;
}

///
pub mod re {
    pub use addr;
    pub use anyhow;
    pub use base64;
    pub use lettre;
    pub use libc;
    pub use log;
    pub use serde_json;
    pub use strum;
    pub use tokio;
    pub use vsmtp_rsasl;
}

#[doc(hidden)]
#[macro_export]
macro_rules! collection {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*]))
    }};
    // set-like
    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$($v,)*]))
    }};
}
