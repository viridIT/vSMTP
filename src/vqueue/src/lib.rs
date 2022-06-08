//! vQueue: the vSMTP's queue manager

#![doc(html_no_source)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::multiple_crate_versions)]

mod args;
mod command;
mod model;

pub use args::{Args, Commands, MessageCommand, MessageShowFormat};
pub use command::execute;
pub(crate) use model::{QueueContent, QueueEntry};
