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

/// Codes as the start of each lines of a reply
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum ReplyCode {
    /// simple Reply Code as defined in RFC5321
    Code {
        // https://datatracker.ietf.org/doc/html/rfc5321#section-4.2
        // NOTE: could be a struct with 3 digits
        /// code base
        code: u16,
    },
    /// enhanced codes
    Enhanced {
        // https://datatracker.ietf.org/doc/html/rfc5321#section-4.2
        // NOTE: could be a struct with 3 digits
        /// code base
        code: u16,
        ///
        // NOTE: could be a struct with 3 digits
        enhanced: String,
    },
}

impl ReplyCode {
    ///
    #[must_use]
    pub fn is_error(&self) -> bool {
        match self {
            ReplyCode::Code { code } | ReplyCode::Enhanced { code, .. } => code / 100 >= 4,
        }
    }

    fn try_parse<'a>(self, words: &[&str], line: &'a str) -> anyhow::Result<(Self, &'a str)> {
        match (self, words) {
            (Self::Enhanced { .. }, [_, "", ..]) => anyhow::bail!("empty second words"),
            (Self::Enhanced { .. }, [code, enhanced, ..]) => {
                let enhanced_len = enhanced.len();
                let enhanced = enhanced
                    .splitn(3, '.')
                    .map(|s| {
                        s.parse::<u16>()?;
                        Ok(s.to_string())
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .join(".");

                Ok((
                    Self::Enhanced {
                        code: code.parse::<u16>()?,
                        enhanced,
                    },
                    {
                        let mut line = &line[code.len() + 1 + enhanced_len..];
                        if line.starts_with(' ') {
                            line = &line[1..];
                        }
                        line
                    },
                ))
            }
            (Self::Code { .. }, [code, ..]) => Ok((
                Self::Code {
                    code: code.parse::<u16>()?,
                },
                {
                    let mut line = &line[code.len()..];
                    if line.starts_with(' ') {
                        line = &line[1..];
                    }
                    line
                },
            )),
            _ => anyhow::bail!("invalid data {line}"),
        }
    }

    ///
    /// # Errors
    ///
    /// * not the right format
    pub fn parse(line: &str) -> anyhow::Result<(Self, &'_ str)> {
        let words = line.split(' ').collect::<Vec<&str>>();
        for i in [
            Self::Enhanced {
                code: u16::default(),
                enhanced: String::default(),
            },
            Self::Code {
                code: u16::default(),
            },
        ] {
            let output = i.try_parse(words.as_slice(), line);
            if output.is_ok() {
                return output;
            }
        }
        anyhow::bail!("invalid format {words:?}");
    }
}

impl std::fmt::Display for ReplyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyCode::Code { code } => f.write_fmt(format_args!("{code}")),
            ReplyCode::Enhanced { code, enhanced } => {
                f.write_fmt(format_args!("{code} {enhanced}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ReplyCode;

    #[test]
    fn display() {
        assert_eq!(
            format!("{}", ReplyCode::Code { code: 250 }),
            "250".to_string()
        );

        assert_eq!(
            format!(
                "{}",
                ReplyCode::Enhanced {
                    code: 504,
                    enhanced: "5.5.4".to_string()
                }
            ),
            "504 5.5.4".to_string()
        );
    }

    #[test]
    fn parse() {
        assert_eq!(
            ReplyCode::parse("250").unwrap(),
            (ReplyCode::Code { code: 250 }, "")
        );
        assert_eq!(
            ReplyCode::parse("504 ").unwrap(),
            (ReplyCode::Code { code: 504 }, "")
        );
        assert_eq!(
            ReplyCode::parse("220 {domain} ESMTP Service ready").unwrap(),
            (
                ReplyCode::Code { code: 220 },
                "{domain} ESMTP Service ready"
            )
        );

        assert_eq!(
            ReplyCode::parse("504 5.5.4").unwrap(),
            (
                ReplyCode::Enhanced {
                    code: 504,
                    enhanced: "5.5.4".to_string()
                },
                ""
            )
        );
        assert_eq!(
            ReplyCode::parse("504 5.5.4 ").unwrap(),
            (
                ReplyCode::Enhanced {
                    code: 504,
                    enhanced: "5.5.4".to_string()
                },
                ""
            )
        );
        assert_eq!(
            ReplyCode::parse("451 5.7.3 STARTTLS is required to send mail").unwrap(),
            (
                ReplyCode::Enhanced {
                    code: 451,
                    enhanced: "5.7.3".to_string()
                },
                "STARTTLS is required to send mail"
            )
        );
    }
}
