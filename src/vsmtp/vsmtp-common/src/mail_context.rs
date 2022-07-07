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

    /// get the value of an header, return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        match self {
            Self::Raw { headers, .. } => {
                for header in headers {
                    let mut split = header.splitn(2, ": ");
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
