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

use crate::{Either, Mail, MailParser};

/// Representation of a mail
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RawBody {
    headers: Vec<String>,
    body: Option<String>,
}

impl RawBody {
    /// Return an iterator over the headers field
    pub fn headers(&self) -> impl Iterator<Item = &str> {
        self.headers.iter().map(String::as_str)
    }

    /// Return an iterator over the body, line by line
    #[must_use]
    pub fn body(&self) -> Option<impl Iterator<Item = &str>> {
        self.body.as_ref().map(|s| s.lines())
    }

    #[must_use]
    fn get_header(&self, name: &str) -> Option<String> {
        for (idx, header) in self.headers.iter().enumerate() {
            if header.starts_with(' ') || header.starts_with('\t') {
                continue;
            }
            let mut split = header.splitn(2, ':');
            match (split.next(), split.next()) {
                (Some(key), Some(value)) if key.to_lowercase() == name.to_lowercase() => {
                    let mut s = value.to_string();
                    for i in self.headers[idx + 1..]
                        .iter()
                        .take_while(|s| s.starts_with(' ') || s.starts_with('\t'))
                    {
                        s.push_str(i);
                    }
                    return Some(s);
                }
                (Some(_), Some(_)) => continue,
                _ => break,
            }
        }

        None
    }

    fn set_header(&mut self, name: &str, value: &str) {
        for header in &mut self.headers {
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

    fn add_header(&mut self, name: &str, value: &str) {
        // TODO: handle folding ?
        self.headers.push(format!("{name}: {value}"));
    }

    fn prepend_header(&mut self, name: &str, value: &str) {
        // TODO: handle folding ?
        self.headers.splice(..0, [format!("{name}: {value}")]);
    }
}

/// Message body issued by a SMTP transaction
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MessageBody {
    raw: RawBody,
    parsed: Option<Mail>,
}

impl Default for MessageBody {
    fn default() -> Self {
        Self {
            raw: RawBody {
                headers: vec![],
                body: None,
            },
            parsed: None,
        }
    }
}

impl std::fmt::Display for MessageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in &self.raw.headers {
            f.write_str(i)?;
            f.write_str("\r\n")?;
        }
        f.write_str("\r\n")?;
        if let Some(body) = &self.raw.body {
            f.write_str(body)?;
        }
        Ok(())
    }
}

impl MessageBody {
    ///
    #[must_use]
    pub fn new(headers: &[&str], body: &str) -> Self {
        Self {
            raw: RawBody {
                headers: headers.iter().map(ToString::to_string).collect(),
                body: Some(body.to_string()),
            },
            parsed: None,
        }
    }

    ///
    #[must_use]
    pub const fn inner(&self) -> &RawBody {
        &self.raw
    }

    /// Does the instance contains a parsed part ?
    #[must_use]
    pub const fn has_parsed(&self) -> bool {
        self.parsed.is_some()
    }

    /// # Errors
    ///
    /// * failed to create the folder in `queues_dirpath`
    pub fn write_to_mails(
        &self,
        queues_dirpath: &std::path::Path,
        message_id: &str,
    ) -> std::io::Result<()> {
        let buf = std::path::PathBuf::from(queues_dirpath).join("mails");
        if !buf.exists() {
            std::fs::DirBuilder::new().recursive(true).create(&buf)?;
        }
        let mut to_write = buf.join(message_id);
        // to_write.set_extension(match &self {
        //     MessageBody::Raw { .. } => "eml",
        //     MessageBody::Parsed(_) => "json",
        // });
        todo!();

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&to_write)?;

        // std::io::Write::write_all(
        //     &mut file,
        //     match self {
        //         MessageBody::Raw { .. } => message.to_string(),
        //         MessageBody::Parsed(parsed) => serde_json::to_string(parsed)?,
        //     }
        //     .as_bytes(),
        // )?;

        Ok(())
    }

    fn stringify(&self) -> Option<String> {
        self.parsed.as_ref().map(ToString::to_string)
    }

    /// get the value of an header, return None if it does not exists or when the body is empty.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<String> {
        self.parsed.as_ref().map_or_else(
            || self.raw.get_header(name),
            |p| p.get_header(name).map(str::to_string),
        )
    }

    /// rewrite a header with a new value or add it to the header section.
    pub fn set_header(&mut self, name: &str, value: &str) {
        if let Some(parsed) = &mut self.parsed {
            parsed.set_header(name, value);
        }

        self.raw.set_header(name, value);
    }

    /// prepend a header to the header section.
    pub fn add_header(&mut self, name: &str, value: &str) {
        if let Some(parsed) = &mut self.parsed {
            parsed.prepend_headers([(name.to_string(), value.to_string())]);
        }

        self.raw.add_header(name, value);
    }

    ///
    pub fn take_headers(&mut self) -> Vec<String> {
        // if let MessageBody::Raw { headers, .. } = self {
        //     return std::mem::take(headers);
        // }

        vec![]
    }

    /// # Errors
    ///
    /// * Fail to parse using the provided [`MailParser`]
    pub fn parse<P: MailParser>(&mut self) -> anyhow::Result<()> {
        match P::default().parse_raw(&self.raw)? {
            Either::Left(_) => anyhow::bail!("expected a `mail` in this context, got a `raw`"),
            Either::Right(parsed) => {
                self.parsed = Some(parsed);
                Ok(())
            }
        }
    }

    /// push a header to the header section.
    ///
    /// push back
    pub fn append_header(&mut self, name: &str, value: &str) {
        self.raw.add_header(name, value);

        if let Some(parsed) = &mut self.parsed {
            parsed.push_headers([(name.to_string(), value.to_string())]);
        }
    }

    /// prepend a header to the header section.
    ///
    /// push front
    pub fn prepend_header(&mut self, name: &str, value: &str) {
        if let Some(parsed) = &mut self.parsed {
            parsed.prepend_headers([(name.to_string(), value.to_string())]);
        }

        self.raw.prepend_header(name, value);
    }

    // prepend a set of headers to the header section.
    // fn prepend_raw_headers(&mut self, to_prepend: impl Iterator<Item = String>) {
    //     if let MessageBody::Raw { headers, .. } = self {
    //         headers.splice(..0, to_prepend);
    //     }
    // }
}
