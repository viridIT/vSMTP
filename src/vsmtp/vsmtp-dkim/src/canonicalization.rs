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

///
#[derive(Debug, PartialEq, Eq, Copy, Clone, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
#[allow(clippy::module_name_repetitions)]
pub enum CanonicalizationAlgorithm {
    ///
    Simple,
    ///
    Relaxed,
}

impl CanonicalizationAlgorithm {
    fn trim_whitespace(s: &str) -> String {
        let mut new_str = s.trim().to_owned();
        let mut prev = ' ';
        new_str.retain(|ch| {
            let result = ch != ' ' || prev != ' ';
            prev = ch;
            result
        });
        new_str
    }

    ///
    #[must_use]
    pub fn canonicalize_body(self, input: &str) -> String {
        match self {
            CanonicalizationAlgorithm::Relaxed => {
                let mut s = Self::trim_whitespace(&input.replace('\t', " "));

                while let Some(idx) = s.find(" \r\n") {
                    s.remove(idx);
                }

                while s.ends_with("\r\n\r\n") {
                    s.remove(s.len() - 1);
                    s.remove(s.len() - 1);
                }

                if !s.is_empty() && !s.ends_with("\r\n") {
                    s.push('\r');
                    s.push('\n');
                }

                s
            }
            CanonicalizationAlgorithm::Simple => {
                if input.is_empty() {
                    return "\r\n".to_string();
                }

                let mut i = input;
                while i.ends_with("\r\n\r\n") {
                    i = &i[..i.len() - 2];
                }

                i.to_string()
            }
        }
    }

    ///
    #[must_use]
    pub fn canonicalize_header(self, key: &str, value: &str) -> String {
        match self {
            CanonicalizationAlgorithm::Relaxed => {
                format!(
                    "{}:{}\r\n",
                    key.to_lowercase().trim_end(),
                    Self::trim_whitespace(&value.replace('\t', " "))
                )
            }
            CanonicalizationAlgorithm::Simple => {
                format!("{key}:{value}\r\n")
            }
        }
    }
}

///
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Canonicalization {
    ///
    pub header: CanonicalizationAlgorithm,
    ///
    pub body: CanonicalizationAlgorithm,
}

impl Default for Canonicalization {
    fn default() -> Self {
        Self {
            header: CanonicalizationAlgorithm::Simple,
            body: CanonicalizationAlgorithm::Simple,
        }
    }
}

impl std::str::FromStr for Canonicalization {
    type Err = <CanonicalizationAlgorithm as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (header, body) = s
            .split_once('/')
            .map_or_else(|| (s, None), |(k, v)| (k, Some(v)));

        Ok(Self {
            header: CanonicalizationAlgorithm::from_str(header)?,
            body: body.map_or(
                Ok(CanonicalizationAlgorithm::Simple),
                CanonicalizationAlgorithm::from_str,
            )?,
        })
    }
}
