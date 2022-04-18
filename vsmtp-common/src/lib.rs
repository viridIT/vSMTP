//! vSMTP common definition

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

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

/// email address representation
pub mod address;

/// smtp reply code to client's command
pub mod code;

/// envelop of a transaction
pub mod envelop;

/// parsed command of the client
pub mod event;

/// abstraction of the libc
pub mod libc_abstraction;

/// message body
pub mod mail;

/// content generated by a smtp transaction
pub mod mail_context;

/// mime type
pub mod mime_type;

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

/// smtp related constants.
pub mod smtp;

mod mechanism;

/// Data related to ESMTP Authentication
pub mod auth {
    pub use crate::mechanism::Mechanism;
}

#[cfg(test)]
mod tests {
    mod event;

    mod libc_abstraction;
}

///
pub mod re {
    pub use addr;
    pub use anyhow;
    pub use libc;
    pub use log;
    pub use rsasl;
    pub use serde_json;
    pub use strum;
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
