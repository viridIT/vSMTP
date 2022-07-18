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

/// we use Vec instead of a `HashMap` because header ordering is important.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MailHeaders(pub Vec<(String, String)>);

impl std::fmt::Display for MailHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in self.0.iter().map(|(k, v)| HeaderFoldable(k, v)) {
            write!(f, "{}", i)?;
        }
        Ok(())
    }
}

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

impl Default for BodyType {
    fn default() -> Self {
        Self::Undefined
    }
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
                    f.write_str("\r\n")?;
                }
                Ok(())
            }
            Self::Mime(content) => {
                write!(f, "{content}")
            }
            Self::Undefined => Ok(()),
        }
    }
}

/// Message body representation
#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Mail {
    /// Message body 's headers
    pub headers: MailHeaders,
    /// Message body content
    pub body: BodyType,
}

#[derive(Debug)]
struct HeaderFoldable<'a>(&'a str, &'a str);

impl<'a> std::fmt::Display for HeaderFoldable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = convert_case::Casing::to_case(&self.0, convert_case::Case::Train)
            .replace("Id", "ID")
            .replace("Mime-Version", "MIME-Version")
            .replace("Dkim", "DKIM")
            .replace("Arc", "ARC")
            .replace("Spf", "SPF")
            .replace("X-Ms", "X-MS")
            .replace("X-Vr", "X-VR");

        f.write_str(&key)?;
        f.write_str(": ")?;

        let mut byte_writable = self.1;
        if byte_writable.is_empty() {
            return f.write_str("\r\n");
        }

        let mut prev = key.len() + 2;

        // FIXME: we can fold at 78 chars for simple sentence.
        // but must write a continuous string for base64 encoded values (like dkim)
        while !byte_writable.is_empty() {
            let (left, right) = if byte_writable.len() + prev > 998 {
                byte_writable[..998 - prev]
                    .rfind(char::is_whitespace)
                    .map(|idx| (&byte_writable[..idx], &byte_writable[idx..]))
            } else {
                None
            }
            .unwrap_or((byte_writable, ""));

            f.write_str(left)?;
            f.write_str("\r\n")?;

            byte_writable = right;
            if !byte_writable.is_empty() {
                std::fmt::Write::write_char(f, '\t')?;
                prev = 1;
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.headers)?;

        if !matches!(self.body, BodyType::Mime(_)) {
            f.write_str("\r\n")?;
        }

        write!(f, "{}", self.body)
    }
}

impl Mail {
    /// change the from field of the header
    pub fn rewrite_mail_from(&mut self, value: &str) {
        if let Some((_, old)) = self
            .headers
            .0
            .iter_mut()
            .find(|(header, _)| header.to_lowercase() == "from")
        {
            *old = value.to_string();
        } else {
            self.headers.0.push(("From".to_string(), value.to_string()));
        }
    }

    /// change one recipients value from @old to @new.
    pub fn rewrite_rcpt(&mut self, old: &str, new: &str) {
        if let Some((_, rcpts)) = self
            .headers
            .0
            .iter_mut()
            .find(|(header, _)| header.to_lowercase() == "to")
        {
            *rcpts = rcpts.replace(old, new);
        } else {
            self.headers.0.push(("To".to_string(), new.to_string()));
        }
    }

    /// add a recipients
    pub fn add_rcpt(&mut self, new: &str) {
        if let Some((_, rcpts)) = self
            .headers
            .0
            .iter_mut()
            .find(|(header, _)| header.to_lowercase() == "to")
        {
            *rcpts = format!("{rcpts}, {new}");
        } else {
            self.headers.0.push(("To".to_string(), new.to_string()));
        }
    }

    /// remove a recipients
    pub fn remove_rcpt(&mut self, old: &str) {
        self.headers
            .0
            .iter_mut()
            .find(|(header, _)| header.to_lowercase() == "to")
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
        if let Some((_, old_value)) = self.headers.0.iter_mut().find(|(header, _)| header == name) {
            *old_value = value.to_string();
        } else {
            self.headers.0.push((name.to_string(), value.to_string()));
        }
    }

    /// get the value of an header, return None if it does not exists.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .0
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    /// get the value of an header starting from the end,
    /// return None if it does not exists.
    #[must_use]
    pub fn get_header_rev(&self, name: &str) -> Option<&str> {
        self.headers
            .0
            .iter()
            .rev()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    // NOTE: would a double ended queue / linked list interesting in this case ?
    /// prepend new headers to the email.
    pub fn prepend_headers(&mut self, headers: impl IntoIterator<Item = (String, String)>) {
        self.headers.0.splice(..0, headers);
    }

    /// push new headers to the email.
    pub fn push_headers(&mut self, headers: impl IntoIterator<Item = (String, String)>) {
        self.headers.0.extend(headers);
    }
}

#[cfg(test)]
mod test {

    use crate::{MimeBodyType, MimeHeader};

    use super::*;

    #[test]
    fn test_construct_mail() {
        let empty_mail = Mail {
            headers: MailHeaders(vec![("From".to_string(), "a@a".to_string())]),
            body: BodyType::Undefined,
        };

        // on newline added to separate the body, one for the empty body.
        // anyway, this example should not happen in a real scenario.
        assert_eq!(format!("{empty_mail}"), "From: a@a\r\n\r\n".to_string());

        let regular_mail = Mail {
            headers: MailHeaders(vec![("From".to_string(), "a@a".to_string())]),
            body: BodyType::Regular(vec!["This is a regular body.".to_string()]),
        };

        assert_eq!(
            format!("{regular_mail}"),
            "From: a@a\r\n\r\nThis is a regular body.\r\n".to_string()
        );

        let mime_mail = Mail {
            headers: MailHeaders(vec![
                ("From".to_string(), "a@a".to_string()),
                ("Mime-Version".to_string(), "1.0".to_string()),
            ]),
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
            [
                "From: a@a\r\n",
                "MIME-Version: 1.0\r\n",
                "Content-Type: text/plain\r\n",
                "\r\n",
                "this is a regular mime body.\r\n",
            ]
            .concat()
        );
    }

    #[test]
    fn test_append_headers() {
        let mut mail = Mail {
            body: BodyType::Regular(vec!["email content".to_string()]),
            ..Mail::default()
        };

        mail.push_headers(vec![
            ("Subject".to_string(), "testing an email".to_string()),
            ("MIME-Version".to_string(), "1.0".to_string()),
        ]);

        assert_eq!(
            format!("{mail}"),
            [
                "Subject: testing an email\r\n",
                "MIME-Version: 1.0\r\n",
                "\r\n",
                "email content\r\n"
            ]
            .concat()
        );

        mail.prepend_headers(vec![
            ("From".to_string(), "b@b".to_string()),
            (
                "Date".to_string(),
                "tue, 30 nov 2021 20:54:27 +0100".to_string(),
            ),
            ("To".to_string(), "john@doe.com, green@foo.bar".to_string()),
        ]);

        assert_eq!(
            format!("{mail}"),
            [
                "From: b@b\r\n",
                "Date: tue, 30 nov 2021 20:54:27 +0100\r\n",
                "To: john@doe.com, green@foo.bar\r\n",
                "Subject: testing an email\r\n",
                "MIME-Version: 1.0\r\n",
                "\r\n",
                "email content\r\n"
            ]
            .concat()
        );
    }

    #[test]
    fn test_rcpt_mutation() {
        let mut mail = Mail::default();

        // rewrite when the header does not exists inserts the header.
        mail.rewrite_mail_from("a@a");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![("From".to_string(), "a@a".to_string())])
        );

        mail.rewrite_mail_from("b@b");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![("From".to_string(), "b@b".to_string())])
        );

        mail.rewrite_rcpt("b@b", "a@a");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![
                ("From".to_string(), "b@b".to_string()),
                ("To".to_string(), "a@a".to_string())
            ])
        );

        mail.add_rcpt("green@foo.bar");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![
                ("From".to_string(), "b@b".to_string()),
                ("To".to_string(), "a@a, green@foo.bar".to_string())
            ])
        );

        mail.rewrite_rcpt("a@a", "john@doe");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![
                ("From".to_string(), "b@b".to_string()),
                ("To".to_string(), "john@doe, green@foo.bar".to_string())
            ])
        );

        mail.remove_rcpt("john@doe");
        assert_eq!(
            mail.headers,
            MailHeaders(vec![
                ("From".to_string(), "b@b".to_string()),
                ("To".to_string(), "green@foo.bar".to_string())
            ])
        );
    }
}
