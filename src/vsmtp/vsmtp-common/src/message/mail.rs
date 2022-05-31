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
use super::mime_type::Mime;

/// we use Vec instead of a HashMap because header ordering is important.
#[allow(clippy::module_name_repetitions)]
pub type MailHeaders = Vec<(String, String)>;

/// see rfc5322 (section 2.1 and 2.3)
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[allow(clippy::use_self)]
pub enum BodyType {
    /// Text message body
    Regular(Vec<String>),
    /// Mime
    Mime(Box<Mime>),
    /// Empty message body
    Undefined,
}

impl std::fmt::Display for BodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regular(content) => {
                for i in content {
                    if i.starts_with('.') {
                        std::fmt::Write::write_char(f, '.')?;
                    }
                    f.write_str(i)?;
                }
                Ok(())
            }
            Self::Mime(content) => write!(f, "{content}"),
            Self::Undefined => Ok(()),
        }
    }
}

/// Message body representation
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Mail {
    /// Message body 's headers
    pub headers: MailHeaders,
    /// Message body content
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

impl std::fmt::Display for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, value) in &self.headers {
            writeln!(f, "{}: {}", key, value)?;
        }
        if !matches!(self.body, BodyType::Mime(_)) {
            writeln!(f)?;
        }

        write!(f, "{}", self.body)
    }
}

impl Mail {
    /// change the from field of the header
    pub fn rewrite_mail_from(&mut self, value: &str) {
        if let Some((_, old)) = self.headers.iter_mut().find(|(header, _)| header == "from") {
            *old = value.to_string();
        } else {
            self.headers.push(("from".to_string(), value.to_string()));
        }
    }

    /// change one recipients value from @old to @new.
    pub fn rewrite_rcpt(&mut self, old: &str, new: &str) {
        if let Some((_, rcpts)) = self.headers.iter_mut().find(|(header, _)| header == "to") {
            *rcpts = rcpts.replace(old, new);
        } else {
            self.headers.push(("to".to_string(), new.to_string()));
        }
    }

    /// add a recipients
    pub fn add_rcpt(&mut self, new: &str) {
        if let Some((_, rcpts)) = self.headers.iter_mut().find(|(header, _)| header == "to") {
            *rcpts = format!("{rcpts}, {new}");
        } else {
            self.headers.push(("to".to_string(), new.to_string()));
        }
    }

    /// remove a recipients
    pub fn remove_rcpt(&mut self, old: &str) {
        self.headers
            .iter_mut()
            .find(|(header, _)| header == "to")
            .and_then::<(), _>(|(_, rcpts)| {
                if rcpts.find(old) == Some(0) {
                    *rcpts = rcpts.replace(format!("{old}, ").as_str(), "");
                } else {
                    *rcpts = rcpts.replace(format!(", {old}").as_str(), "");
                }
                None
            });
    }

    /// rewrite a header with a new value or push it to the header stack.
    pub fn set_header(&mut self, name: &str, value: &str) {
        if let Some((_, old_value)) = self.headers.iter_mut().find(|(header, _)| header == name) {
            *old_value = value.to_string();
        } else {
            self.headers.push((name.to_string(), value.to_string()));
        }
    }

    /// get the value of an header, return None if it does not exists.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    /// prepend new headers to the email, folding if necessary.
    pub fn prepend_headers(&mut self, headers: impl IntoIterator<Item = (String, String)>) {
        self.headers.splice(..0, headers);
    }

    /// push new headers to the email, folding if necessary.
    pub fn push_headers(&mut self, headers: impl IntoIterator<Item = (String, String)>) {
        self.headers.extend(headers);
    }
}

#[cfg(test)]
mod test {

    use crate::{MimeBodyType, MimeHeader};

    use super::*;

    #[test]
    fn test_construct_mail() {
        let empty_mail = Mail {
            headers: vec![("from".to_string(), "a@a".to_string())],
            body: BodyType::Undefined,
        };

        // on newline added to separate the body, one for the empty body.
        // anyway, this example should not happen in a real scenario.
        assert_eq!(
            format!("{empty_mail}"),
            r#"from: a@a

"#
            .to_string()
        );

        let regular_mail = Mail {
            headers: vec![("from".to_string(), "a@a".to_string())],
            body: BodyType::Regular(vec!["This is a regular body.".to_string()]),
        };

        assert_eq!(
            format!("{regular_mail}"),
            r#"from: a@a

This is a regular body."#
                .to_string()
        );

        let mime_mail = Mail {
            headers: vec![
                ("from".to_string(), "a@a".to_string()),
                ("mime-version".to_string(), "1.0".to_string()),
            ],
            body: BodyType::Mime(Box::new(Mime {
                headers: vec![MimeHeader {
                    name: "content-type".to_string(),
                    value: "text/plain".to_string(),
                    args: std::collections::HashMap::new(),
                }],
                content: MimeBodyType::Regular(vec!["this is a regular mime body.".to_string()]),
            })),
        };

        // mime headers should be merged with the rfc822 message header section.
        assert_eq!(
            format!("{mime_mail}"),
            r#"from: a@a
mime-version: 1.0
content-type: text/plain

this is a regular mime body.
"#
            .to_string()
        );
    }

    #[test]
    fn test_add_headers() {
        let mut mail = Mail {
            body: BodyType::Regular(vec!["email content".to_string()]),
            ..Mail::default()
        };

        mail.push_headers(vec![
            ("subject".to_string(), "testing an email".to_string()),
            ("mime-version".to_string(), "1.0".to_string()),
        ]);

        assert_eq!(
            format!("{mail}"),
            r#"subject: testing an email
mime-version: 1.0

email content"#
                .to_string()
        );

        mail.prepend_headers(vec![
            ("from".to_string(), "b@b".to_string()),
            (
                "date".to_string(),
                "tue, 30 nov 2021 20:54:27 +0100".to_string(),
            ),
            ("to".to_string(), "john@doe.com, green@foo.bar".to_string()),
        ]);

        assert_eq!(
            format!("{mail}"),
            r#"from: b@b
date: tue, 30 nov 2021 20:54:27 +0100
to: john@doe.com, green@foo.bar
subject: testing an email
mime-version: 1.0

email content"#
                .to_string()
        );
    }

    #[test]
    fn test_rcpt_mutation() {
        let mut mail = Mail::default();

        // rewrite when the header does not exists inserts the header.
        mail.rewrite_mail_from("a@a");
        assert_eq!(mail.headers, vec![("from".to_string(), "a@a".to_string())]);

        mail.rewrite_mail_from("b@b");
        assert_eq!(mail.headers, vec![("from".to_string(), "b@b".to_string())]);

        mail.rewrite_rcpt("b@b", "a@a");
        assert_eq!(
            mail.headers,
            vec![
                ("from".to_string(), "b@b".to_string()),
                ("to".to_string(), "a@a".to_string())
            ]
        );

        mail.add_rcpt("green@foo.bar");
        assert_eq!(
            mail.headers,
            vec![
                ("from".to_string(), "b@b".to_string()),
                ("to".to_string(), "a@a, green@foo.bar".to_string())
            ]
        );

        mail.rewrite_rcpt("a@a", "john@doe");
        assert_eq!(
            mail.headers,
            vec![
                ("from".to_string(), "b@b".to_string()),
                ("to".to_string(), "john@doe, green@foo.bar".to_string())
            ]
        );

        mail.remove_rcpt("john@doe");
        assert_eq!(
            mail.headers,
            vec![
                ("from".to_string(), "b@b".to_string()),
                ("to".to_string(), "green@foo.bar".to_string())
            ]
        );
    }
}
