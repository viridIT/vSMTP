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

/// Representation of a mail
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RawBody {
    headers: Vec<String>,
    body: Option<String>,
}

impl RawBody {
    ///
    #[must_use]
    pub fn new(headers: Vec<String>, body: String) -> Self {
        Self {
            headers,
            body: Some(body),
        }
    }

    /// Return an iterator over the headers field
    pub fn headers(&self) -> impl Iterator<Item = &str> {
        self.headers.iter().map(String::as_str)
    }

    /// Return an iterator over the body, line by line
    #[must_use]
    pub fn body(&self) -> Option<impl Iterator<Item = &str>> {
        self.body.as_ref().map(|s| s.lines())
    }

    ///
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<String> {
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

    ///
    pub fn set_header(&mut self, name: &str, value: &str) {
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

    ///
    pub fn add_header(&mut self, name: &str, value: &str) {
        // TODO: handle folding ?
        self.headers.push(format!("{name}: {value}"));
    }

    ///
    pub fn prepend_header(&mut self, headers: impl IntoIterator<Item = String>) {
        // TODO: handle folding ?
        self.headers.splice(..0, headers);
    }
}

impl std::fmt::Display for RawBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in &self.headers {
            f.write_str(i)?;
            f.write_str("\r\n")?;
        }
        f.write_str("\r\n")?;
        if let Some(body) = &self.body {
            f.write_str(body)?;
        }
        Ok(())
    }
}
