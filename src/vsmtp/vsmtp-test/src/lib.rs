//! vSMTP testing utilities

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

/// Config shortcut
pub mod config;

///
pub mod receiver;

///
pub mod get_tls_file;

#[cfg(test)]
mod tests;
