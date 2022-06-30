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
mod delivery;
mod processing;
mod receiver;
mod runtime;
mod server;

pub use receiver::MailHandler;

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{handle_connection, AbstractIO, Connection, OnMail};
pub use runtime::start_runtime;
pub use server::{socket_bind_anyhow, Server};

use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        serde_json, tokio,
    },
    MessageBody,
};

pub(crate) async fn context_from_file_path(file: &std::path::Path) -> anyhow::Result<MailContext> {
    let content = tokio::fs::read_to_string(&file)
        .await
        .with_context(|| format!("Cannot read file '{}'", file.display()))?;

    serde_json::from_str::<MailContext>(&content)
        .with_context(|| format!("Cannot deserialize: '{content:?}'"))
}

/// Return a message body from a file path.
/// Try to parse the file as JSON, if it fails, try to parse it as plain text.
///
/// # Errors
///
/// * file(s) not found
/// * file found but failed to read
/// * file read but failed to serialize
pub async fn message_from_file_path(
    mut filepath: std::path::PathBuf,
) -> anyhow::Result<MessageBody> {
    filepath.set_extension("json");
    if filepath.exists() {
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .with_context(|| format!("Cannot read file '{}'", filepath.display()))?;

        return serde_json::from_str::<MessageBody>(&content)
            .with_context(|| format!("Cannot deserialize: '{content:?}'"));
    }

    filepath.set_extension("eml");
    if filepath.exists() {
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .with_context(|| format!("Cannot read file '{}'", filepath.display()))?;

        let (headers, body) = content
            .split_once("\r\n\r\n")
            .ok_or_else(|| anyhow::anyhow!("Cannot find message body"))?;

        return Ok(MessageBody::Raw {
            headers: headers.lines().map(str::to_string).collect(),
            body: Some(body.to_string()),
        });
    }
    anyhow::bail!("failed does not exist")
}
