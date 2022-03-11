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
#![allow(clippy::future_not_send)]

///
pub mod processes;
///
pub mod queue;
///
pub mod receiver;
///
pub mod resolver;
///
pub mod server;
mod tls_helpers;
