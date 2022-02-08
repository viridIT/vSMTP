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

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
/// see rfc5322 (section 2.1 and 2.3)
pub enum BodyType {
    Regular(Vec<String>),
    Mime(Box<Mime>),
    Undefined,
}

impl ToString for BodyType {
    fn to_string(&self) -> String {
        match self {
            Self::Regular(content) => content.join("\n"),
            Self::Mime(content) => {
                let (headers, body) = content.to_raw();
                format!("{}\n{}", headers, body)
            }
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
    /// return raw string of the mail with (headers, body).
    pub fn to_raw(&self) -> (String, String) {
        (
            self.headers
                .iter()
                .map(|(header, value)| format!("{}: {}", header, value))
                .collect::<Vec<_>>()
                .join("\n"),
            self.body.to_string(),
        )
    }

    pub fn rewrite_from(&mut self, value: &str) {
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

    pub fn delete_rcpt(&mut self, old: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "to")
            .and_then::<(), _>(|(_, rcpts)| {
                *rcpts = rcpts.replace(format!(", {old}").as_str(), "");
                None
            });
    }
}
