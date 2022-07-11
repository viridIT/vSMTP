use anyhow::Context;

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
use crate::{auth::Credentials, envelop::Envelop, status::Status, Mail, MailParser};

/// average size of a mail
pub const MAIL_CAPACITY: usize = 10_000_000; // 10MB

/// metadata
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MessageMetadata {
    /// instant when the last "MAIL FROM" has been received.
    pub timestamp: std::time::SystemTime,
    /// unique id generated when the "MAIL FROM" has been received.
    /// format: {mail timestamp}{connection timestamp}{process id}
    pub message_id: String,
    /// whether further rule analysis has been skipped.
    pub skipped: Option<Status>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            message_id: String::default(),
            skipped: None,
        }
    }
}

/// Message body issued by a SMTP transaction
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum MessageBody {
    /// The raw representation of the message
    Raw {
        /// The headers of the top level message
        headers: Vec<String>,
        /// Complete body of the message
        body: Option<String>,
    },
    /// The message parsed using a [`MailParser`]
    Parsed(Box<Mail>),
}

impl Default for MessageBody {
    fn default() -> Self {
        Self::Raw {
            headers: vec![],
            body: None,
        }
    }
}

impl std::fmt::Display for MessageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw { headers, body } => {
                for i in headers {
                    f.write_str(i)?;
                    f.write_str("\r\n")?;
                }
                f.write_str("\r\n")?;
                f.write_str(body.as_ref().map_or("", std::string::String::as_str))
            }
            Self::Parsed(mail) => f.write_fmt(format_args!("{mail}")),
        }
    }
}

impl MessageBody {
    /// Return a message body from a file path.
    /// Try to parse the file as JSON, if it fails, try to parse it as plain text.
    ///
    /// # Errors
    ///
    /// * file(s) not found
    /// * file found but failed to read
    /// * file read but failed to serialize
    pub async fn from_file_path(mut filepath: std::path::PathBuf) -> anyhow::Result<MessageBody> {
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

    ///
    pub fn take_headers(&mut self) -> Vec<String> {
        if let MessageBody::Raw { headers, .. } = self {
            return std::mem::take(headers);
        }

        vec![]
    }

    /// Create a new instance of [`MessageBody::Parsed`], cloning if already parsed
    ///
    /// # Errors
    ///
    /// * Fail to parse using the provided [`MailParser`]
    pub fn to_parsed<P: MailParser>(&mut self) -> anyhow::Result<()> {
        if let Self::Raw { headers, body } = self {
            *self =
                P::default().parse_raw(std::mem::take(headers), body.take().unwrap_or_default())?;
        }
        Ok(())
    }

    /// return the first header that match the `needle` parameter in the `haystack` iterator.
    /// FIXME: handle header folding.
    #[must_use]
    fn get_raw_header<'a>(
        haystack: impl Iterator<Item = &'a String>,
        needle: &str,
    ) -> Option<&'a str> {
        for header in haystack {
            let mut split = header.splitn(2, ": ");
            match (split.next(), split.next()) {
                (Some(header), Some(value)) if header == needle => {
                    return Some(value);
                }
                _ => continue,
            }
        }

        None
    }

    /// get the value of an header, checking from up to bottom.
    /// Return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        match self {
            Self::Raw { headers, .. } => Self::get_raw_header(headers.iter(), name),
            Self::Parsed(parsed) => parsed.get_header(name),
        }
    }

    /// Get the value of an header, checking from bottom to up.
    /// Return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header_rev(&self, name: &str) -> Option<&str> {
        match self {
            Self::Raw { headers, .. } => Self::get_raw_header(headers.iter().rev(), name),
            Self::Parsed(parsed) => parsed.get_header_rev(name),
        }
    }

    /// rewrite a header with a new value or add it to the header section.
    pub fn set_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Raw { headers, .. } => {
                for header in headers {
                    let mut split = header.splitn(2, ": ");
                    match (split.next(), split.next()) {
                        (Some(key), Some(_)) if key == name => {
                            // TODO: handle folding ?
                            *header = format!("{key}: {value}");
                            return;
                        }
                        _ => {}
                    }
                }
                self.append_header(name, value);
            }
            Self::Parsed(parsed) => parsed.set_header(name, value),
        }
    }

    /// push a header to the header section.
    pub fn append_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Raw { headers, .. } => {
                // TODO: handle folding ?
                headers.push(format!("{name}: {value}"));
            }
            Self::Parsed(parsed) => {
                parsed.push_headers([(name.to_string(), value.to_string())]);
            }
        }
    }

    /// prepend a header to the header section.
    pub fn prepend_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Raw { headers, .. } => {
                // TODO: handle folding ?
                headers.splice(..0, [format!("{name}: {value}")]);
            }
            Self::Parsed(parsed) => {
                parsed.prepend_headers([(name.to_string(), value.to_string())]);
            }
        }
    }

    /// prepend a set of headers to the header section.
    pub fn prepend_raw_headers(&mut self, to_prepend: impl Iterator<Item = String>) {
        if let MessageBody::Raw { headers, .. } = self {
            headers.splice(..0, to_prepend);
        }
    }
}

/// Representation of one connection
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConnectionContext {
    /// time of connection by the client.
    pub timestamp: std::time::SystemTime,
    /// credentials of the client.
    pub credentials: Option<Credentials>,
    /// server's domain of the connection. (from config.server.domain or sni)
    pub server_name: String,
    /// server socket used for this connexion.
    pub server_address: std::net::SocketAddr,
    /// is the client authenticated ?
    pub is_authenticated: bool,
    /// is the connection under tls ?
    pub is_secured: bool,
}

/// Representation of one mail obtained by a transaction SMTP
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MailContext {
    /// information of the connection producing this message
    pub connection: ConnectionContext,
    /// emitter of the mail
    pub client_addr: std::net::SocketAddr,
    /// envelop of the message
    pub envelop: Envelop,
    /// metadata
    pub metadata: Option<MessageMetadata>,
}

impl MailContext {
    fn from_json(content: &str) -> anyhow::Result<Self> {
        serde_json::from_str::<MailContext>(content)
            .with_context(|| format!("Cannot deserialize: '{content:?}'"))
    }

    /// Return a mail context from a file path.
    ///
    /// # Errors
    ///
    /// * file not found.
    /// * file found but failed to read.
    /// * file read but failed to serialize.
    pub async fn from_file_path(file: &std::path::Path) -> anyhow::Result<MailContext> {
        let content = tokio::fs::read_to_string(&file)
            .await
            .with_context(|| format!("Cannot read file '{}'", file.display()))?;

        Self::from_json(&content)
    }

    /// Return a mail context from a file path.
    ///
    /// # Errors
    ///
    /// * file not found.
    /// * file found but failed to read.
    /// * file read but failed to serialize.
    pub fn from_file_path_sync(file: &std::path::Path) -> anyhow::Result<MailContext> {
        let content = std::fs::read_to_string(&file)
            .with_context(|| format!("Cannot read file '{}'", file.display()))?;

        Self::from_json(&content)
    }
}
