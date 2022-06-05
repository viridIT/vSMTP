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
use crate::{envelop::Envelop, status::Status, Mail, MailParser};

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
    Raw(Vec<String>),
    /// The message parsed using a [`MailParser`]
    Parsed(Box<Mail>),
}

impl std::fmt::Display for MessageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(data) => {
                for i in data {
                    f.write_str(i)?;
                    f.write_str("\r\n")?;
                }
                Ok(())
            }
            Self::Parsed(mail) => f.write_fmt(format_args!("{mail}")),
        }
    }
}

impl MessageBody {
    /// Convert a the instance into a [`MessageBody::Parsed`]
    ///
    /// # Errors
    ///
    /// * Fail to parse using the provided [`MailParser`]
    pub fn to_parsed<P: MailParser>(self) -> anyhow::Result<Self> {
        Ok(match self {
            Self::Raw(raw) => P::default().parse(raw)?,
            otherwise @ Self::Parsed(_) => otherwise,
        })
    }

    /// Has the instance been parsed
    #[must_use]
    pub const fn is_parsed(&self) -> bool {
        match self {
            MessageBody::Raw(_) => false,
            MessageBody::Parsed(_) => true,
        }
    }

    /// get the value of an header, return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        match self {
            Self::Raw(raw) => {
                for line in raw {
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
            Self::Raw(raw) => {
                // TODO: handle folded header, but at this point the function should parse the mail...

                for line in raw {
                    let mut split = line.splitn(2, ": ");
                    match (split.next(), split.next()) {
                        (Some(key), Some(_)) if key == name => {
                            *line = format!("{key}: {value}");
                            return;
                        }
                        _ => {}
                    }
                }
                self.add_header(name, value);
            }
            Self::Parsed(parsed) => parsed.set_header(name, value),
        }
    }

    /// prepend a header to the header section.
    pub fn add_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Raw(raw) => {
                raw.splice(..0, [format!("{name}: {value}")]);
            }
            Self::Parsed(parsed) => {
                parsed.prepend_headers([(name.to_string(), value.to_string())]);
            }
        }
    }
}

/// The credentials send by the client, not necessarily the right one
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, strum::Display)]
#[strum(serialize_all = "PascalCase")]
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
    /// verify the token send by anonymous mechanism
    AnonymousToken {
        /// [ email / 1*255TCHAR ]
        token: String,
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
    /// metadata
    pub metadata: Option<MessageMetadata>,
}
