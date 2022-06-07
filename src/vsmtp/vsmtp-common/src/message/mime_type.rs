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
use super::mail::Mail;

/// header of a mime section
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MimeHeader {
    ///
    pub name: String,
    ///
    pub value: String,
    /// parameter ordering does not matter.
    pub args: std::collections::HashMap<String, String>,
}

///
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum MimeBodyType {
    ///
    Regular(Vec<String>),
    ///
    Multipart(MimeMultipart),
    ///
    Embedded(Mail),
}

///
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MimeMultipart {
    ///
    pub preamble: String,
    ///
    pub parts: Vec<Mime>,
    ///
    pub epilogue: String,
}

// TODO: handle folding here
impl std::fmt::Display for MimeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}: {}", self.name, self.value))?;
        if !self.args.is_empty() {
            for (key, value) in &self.args {
                f.write_fmt(format_args!("; {key}=\"{value}\""))?;
            }
        }
        f.write_str("\r\n")?;
        Ok(())
    }
}

struct MimeMultipartDisplayable<'a>(&'a MimeMultipart, &'a str);

impl<'a> std::fmt::Display for MimeMultipartDisplayable<'a> {
    ///  preamble
    ///  --boundary
    ///  *{ headers \n body \n boundary}
    ///  epilogue || nothing
    ///  --end-boundary--
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.0.preamble.is_empty() {
            f.write_fmt(format_args!("{}\r\n", self.0.preamble))?;
        }

        for i in &self.0.parts {
            f.write_fmt(format_args!("--{}\r\n", self.1))?;
            f.write_fmt(format_args!("{i}"))?;
        }

        if !self.0.epilogue.is_empty() {
            f.write_str(&self.0.epilogue)?;
            f.write_str("\r\n")?;
        }
        f.write_fmt(format_args!("--{}--\r\n\r\n", self.1))?;
        Ok(())
    }
}

///
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Mime {
    ///
    pub headers: Vec<MimeHeader>,
    ///
    pub content: MimeBodyType,
}

impl std::fmt::Display for Mime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in &self.headers {
            write!(f, "{i}")?;
        }
        f.write_str("\r\n")?;

        match &self.content {
            MimeBodyType::Regular(regular) => {
                for i in regular {
                    write!(f, "{i}")?;
                    f.write_str("\r\n")?;
                }
                Ok(())
            }
            MimeBodyType::Multipart(multipart) => {
                let boundary = self
                    .headers
                    .iter()
                    .find_map(|header| header.args.get("boundary"))
                    .unwrap();

                write!(f, "{}", MimeMultipartDisplayable(multipart, boundary))
            }
            MimeBodyType::Embedded(mail) => write!(f, "{mail}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type() {
        let input = MimeHeader {
            name: "Content-Type".to_string(),
            value: "text/plain".to_string(),
            args: std::collections::HashMap::from([
                ("charset".to_string(), "us-ascii".to_string()),
                ("another".to_string(), "argument".to_string()),
            ]),
        };

        let order1 = input.to_string()
            == "Content-Type: text/plain; charset=\"us-ascii\"; another=\"argument\"\r\n";
        let order2 = input.to_string()
            == "Content-Type: text/plain; another=\"argument\"; charset=\"us-ascii\"\r\n";

        // arguments can be in any order.
        assert!(order1 || order2, "{input}");

        let input = MimeHeader {
            name: "Content-Type".to_string(),
            value: "application/foobar".to_string(),
            args: std::collections::HashMap::default(),
        };

        assert_eq!(
            input.to_string(),
            "Content-Type: application/foobar\r\n".to_string()
        );
    }
}
