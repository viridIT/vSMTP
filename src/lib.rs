//! vSMTP
#![doc(html_no_source)]
#![deny(missing_docs)]

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

/// ServerConfig, ServerConfigBuilder, default values, and parser
pub mod config;
mod mime;
/// Abstraction of the libc
pub mod my_libc;
mod processes;
mod queue;
/// The transaction receiver, with SMTP state machine
pub mod receiver;
/// The delivery methods supported by the system
pub mod resolver;
mod rules;
/// The main instance of vSMTP
pub mod server;
mod smtp;
mod tls_helpers;

pub use mime::mail::BodyType;
pub use mime::parser::MailMimeParser;
pub use rules::address::Address;
pub use smtp::mail::Body;

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
