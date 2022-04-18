//! vQueue: the vSMTP's queue manager

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
#![allow(clippy::multiple_crate_versions)]

mod args;
mod command;
mod model;

pub use args::{Args, Commands, MessageCommand, MessageShowFormat};
pub use command::execute;
pub use model::{QueueContent, QueueEntry};

/// Generate the list of lifetime
#[must_use]
pub fn lifetimes() -> Vec<u64> {
    (0..9)
        .into_iter()
        .scan(5, |state, _| {
            let out = *state;
            *state *= 2;
            Some(out)
        })
        .collect()
}
