/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use super::mime_type::Mime;

/// we use Vec instead of a HashMap because header ordering is important.
pub type MailHeaders = Vec<(String, String)>;

/// see rfc5322 (section 2.1 and 2.3)
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum BodyType {
    /// Text message body
    Regular(Vec<String>),
    /// Mime
    Mime(Box<Mime>),
    /// Empty message body
    Undefined,
}

impl ToString for BodyType {
    fn to_string(&self) -> String {
        match self {
            Self::Regular(content) => content.join("\n"),
            Self::Mime(content) => content.to_raw(),
            Self::Undefined => String::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Mail {
    pub headers: MailHeaders,
    pub body: BodyType,
}

impl Default for Mail {
    fn default() -> Self {
        Self {
            headers: vec![],
            body: BodyType::Undefined,
        }
    }
}

impl Mail {
    /// get the original header section of the email.
    pub fn raw_headers(&self) -> String {
        self.headers
            .iter()
            .map(|(header, value)| format!("{}: {}", header, value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// get the original body section of the email.
    pub fn raw_body(&self) -> String {
        self.body.to_string()
    }

    /// return the original text representation of the email.
    pub fn to_raw(&self) -> String {
        format!("{}\n\n{}", self.raw_headers(), self.raw_body())
    }

    pub fn rewrite_mail_from(&mut self, value: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "from")
            .and_then::<(), _>(|(_, old)| {
                *old = value.to_string();
                None
            });
    }

    pub fn rewrite_rcpt(&mut self, old: &str, new: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "to")
            .and_then::<(), _>(|(_, rcpts)| {
                *rcpts = rcpts.replace(old, new);
                None
            });
    }

    pub fn add_rcpt(&mut self, new: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "to")
            .and_then::<(), _>(|(_, rcpts)| {
                *rcpts = format!("{}, {}", rcpts, new);
                None
            });
    }

    pub fn remove_rcpt(&mut self, old: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "to")
            .and_then::<(), _>(|(_, rcpts)| {
                *rcpts = rcpts.replace(format!(", {old}").as_str(), "");
                None
            });
    }
}
