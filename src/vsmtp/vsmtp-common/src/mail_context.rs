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
use crate::{envelop::Envelop, mail::Mail, status::Status, MailParser};
use anyhow::Context;

/// average size of a mail
pub const MAIL_CAPACITY: usize = 10_000_000; // 10MB

/// metadata
/// TODO: remove retry & resolver fields.
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
pub enum Body {
    /// Nothing
    Empty,
    /// The raw representation of the message
    Raw(String),
    /// The message parsed using [MailMimeParser]
    Parsed(Box<Mail>),
}

impl std::fmt::Display for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::Empty => "".to_string(),
            Self::Raw(data) => data.clone(),
            Self::Parsed(mail) => mail.to_raw(),
        })
    }
}

impl Body {
    /// Convert a the instance into a [`Body::Parsed`] or [`Body::Empty`]
    ///
    /// # Errors
    ///
    /// * Fail to parse using the provided [`MailParser`]
    pub fn to_parsed<P: MailParser>(self) -> anyhow::Result<Self> {
        Ok(match self {
            Self::Raw(raw) => Self::Parsed(Box::new(P::default().parse(raw.as_bytes())?)),
            otherwise => otherwise,
        })
    }

    /// get the value of an header, return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Raw(raw) => {
                for line in raw.lines() {
                    let mut split = line.splitn(2, ": ");
                    match (split.next(), split.next()) {
                        (Some(header), Some(value)) if header == name => {
                            return Some(value);
                        }
                        (Some(_), Some(_)) => continue,
                        _ => break,
                    }
                }

                None
            }
            Self::Parsed(parsed) => parsed.get_header(name),
        }
    }

    /// rewrite a header with a new value or add it to the header section.
    pub fn set_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Empty => {}
            Self::Raw(raw) => {
                let mut header_start = 0;
                let mut header_end = None;

                for line in raw.lines() {
                    let mut split = line.splitn(2, ": ");
                    match (split.next(), split.next()) {
                        (Some(old_name), Some(_)) if old_name == name => {
                            header_end = Some(line.len());
                            break;
                        }
                        (Some(_), Some(_)) => header_start += line.len() + 1,
                        _ => break,
                    }
                }

                #[allow(clippy::option_if_let_else)]
                if let Some(header_end) = header_end {
                    raw.replace_range(
                        header_start..header_start + header_end,
                        &format!("{name}: {value}"),
                    );
                } else {
                    self.add_header(name, value);
                }
            }
            Self::Parsed(parsed) => parsed.set_header(name, value),
        }
    }

    /// prepend a header to the header section.
    pub fn add_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Empty => {}
            Self::Raw(raw) => *raw = format!("{name}: {value}\n{raw}"),
            Self::Parsed(parsed) => {
                parsed.prepend_headers(vec![(name.to_string(), value.to_string())]);
            }
        }
    }
}

/// The credentials send by the client, not necessarily the right one
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum AuthCredentials {
    /// the pair will be sent and verified by a third party
    Verify {
        ///
        authid: String,
        ///
        authpass: String,
    },
    /// the server will query a third party and make internal verification
    Query {
        ///
        authid: String,
    },
}

/// Representation of one connection
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConnectionContext {
    /// time of connection by the client.
    pub timestamp: std::time::SystemTime,
    /// credentials of the client.
    pub credentials: Option<AuthCredentials>,
    /// server's domain of the connection, (from config.server.domain or sni)
    pub server_name: String,
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
    /// content of the message
    pub body: Body,
    /// metadata
    pub metadata: Option<MessageMetadata>,
}

impl MailContext {
    /// serialize the mail context using serde.
    ///
    /// # Errors
    /// * Failed to read the file
    /// * Failed deserialize to the MailContext struct.
    pub fn from_file<P>(file: P) -> anyhow::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        std::fs::read_to_string(&file)
            .context(format!("Cannot read file '{}'", file.as_ref().display()))
            .map(|content| {
                serde_json::from_str::<Self>(&content)
                    .context(format!("Cannot deserialize: '{}'", content))
            })?
    }
}
