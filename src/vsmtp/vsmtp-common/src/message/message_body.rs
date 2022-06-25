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

use crate::{Mail, MailParser};

/// Message body issued by a SMTP transaction
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum MessageBody {
    /// The raw representation of the message
    Raw {
        /// The headers of the top level message
        headers: Vec<String>,
        /// Complete body of the message
        body: String,
    },
    /// The message parsed using a [`MailParser`]
    Parsed(Box<Mail>),
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
                f.write_str(body)
            }
            Self::Parsed(mail) => f.write_fmt(format_args!("{mail}")),
        }
    }
}

impl MessageBody {
    /// Create a new instance of [`MessageBody::Parsed`], cloning if already parsed
    ///
    /// # Errors
    ///
    /// * Fail to parse using the provided [`MailParser`]
    pub fn to_parsed<P: MailParser>(&mut self) -> anyhow::Result<()> {
        if let Self::Raw { headers, body } = self {
            *self = P::default().parse_raw(std::mem::take(headers), std::mem::take(body))?;
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
                self.add_header(name, value);
            }
            Self::Parsed(parsed) => parsed.set_header(name, value),
        }
    }

    /// prepend a header to the header section.
    pub fn add_header(&mut self, name: &str, value: &str) {
        match self {
            Self::Raw { headers, .. } => {
                // TODO: handle folding ?
                headers.push(format!("{name}: {value}"));
            }
            Self::Parsed(parsed) => {
                parsed.prepend_headers([(name.to_string(), value.to_string())]);
            }
        }
    }
}
