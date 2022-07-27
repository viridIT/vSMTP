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

use crate::ReplyCode;

/// SMTP message send by the server to the client as defined in RFC5321#4.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    code: ReplyCode,
    text: String,
}

impl serde::Serialize for Reply {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.fold())
    }
}

impl<'de> serde::Deserialize<'de> for Reply {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReplyVisitor;

        impl<'de> serde::de::Visitor<'de> for ReplyVisitor {
            type Value = Reply;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("[...]")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Reply::parse_str(v).map_err(serde::de::Error::custom)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                #[derive(serde::Deserialize)]
                #[serde(field_identifier, rename_all = "lowercase")]
                enum Field {
                    Code,
                    Enhanced,
                    Text,
                }

                let mut text: Option<String> = None;
                let mut code = None;
                let mut enhanced = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Code => {
                            if code.is_some() {
                                return Err(serde::de::Error::duplicate_field("code"));
                            }
                            code = Some(map.next_value()?);
                        }
                        Field::Text => {
                            if text.is_some() {
                                return Err(serde::de::Error::duplicate_field("text"));
                            }
                            text = Some(map.next_value()?);
                        }
                        Field::Enhanced => {
                            if enhanced.is_some() {
                                return Err(serde::de::Error::duplicate_field("enhanced"));
                            }
                            enhanced = Some(map.next_value()?);
                        }
                    }
                }
                let code = code.ok_or_else(|| serde::de::Error::missing_field("code"))?;
                Ok(Reply::new(
                    match enhanced {
                        Some(enhanced) => ReplyCode::Enhanced { code, enhanced },
                        None => ReplyCode::Code { code },
                    },
                    text.ok_or_else(|| serde::de::Error::missing_field("text"))?,
                ))
            }
        }

        deserializer.deserialize_any(ReplyVisitor)
    }
}

impl Reply {
    ///
    pub fn new(code: ReplyCode, text: impl Into<String>) -> Self {
        let text = text.into();
        if text.contains("\r\n") {
            log::info!("found a '\\r\\n' in your {code:?}, this is not required as they will be inserted automatically");
        }

        Self { code, text }
    }

    ///
    #[must_use]
    pub const fn code(&self) -> &ReplyCode {
        &self.code
    }

    ///
    #[must_use]
    pub const fn text(&self) -> &String {
        &self.text
    }

    ///
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    ///
    // TODO: should be private and called only when the object is construct,
    // the result should be cached
    #[must_use]
    pub fn fold(&self) -> String {
        let prefix = match &self.code {
            ReplyCode::Code { code } => format!("{code} "),
            ReplyCode::Enhanced { code, enhanced } => format!("{code} {enhanced} "),
        }
        .chars()
        .collect::<Vec<_>>();

        let output = self
            .text
            .split("\r\n")
            .filter(|s| !s.is_empty())
            .flat_map(|line| {
                line.chars()
                    .collect::<Vec<char>>()
                    .chunks(80 - (prefix.len() + 2))
                    .flat_map(|c| [&prefix, c, &"\r\n".chars().collect::<Vec<_>>()].concat())
                    .collect::<String>()
                    .chars()
                    .collect::<Vec<_>>()
            })
            .collect::<String>();

        let mut output = output
            .split("\r\n")
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        let len = output.len();
        for i in &mut output[0..len - 1] {
            i.replace_range(3..4, "-");
        }

        output
            .into_iter()
            .flat_map(|mut l| {
                l.push_str("\r\n");
                l.chars().collect::<Vec<_>>()
            })
            .collect::<String>()
    }

    ///
    /// # Errors
    ///
    /// * @line is not a valid SMTP reply format
    pub fn parse_str(line: &str) -> anyhow::Result<Self> {
        let (code, text) = ReplyCode::parse(line)?;
        Ok(Self::new(code, text.to_string()))
    }

    ///
    #[must_use]
    pub fn combine(informational: &Self, response: &Self) -> Self {
        Self {
            code: response.code.clone(),
            text: format!("{}\r\n{}", informational.text, response.text),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Reply, ReplyCode};

    mod fold {
        use crate::{Reply, ReplyCode};

        #[test]
        fn no_fold() {
            let output = Reply {
                code: ReplyCode::Code { code: 220 },
                text: "this is a custom code.".to_string(),
            }
            .fold();
            pretty_assertions::assert_eq!(output, "220 this is a custom code.\r\n".to_string());
            for i in output.split("\r\n") {
                assert!(i.len() <= 78);
            }
        }

        #[test]
        fn one_line() {
            let output = Reply {
                code: ReplyCode::Enhanced {
                    code: 220,
                    enhanced: "2.0.0".to_string(),
                },
                text: [
                    "this is a long message, a very very long message ...",
                    " carriage return will be properly added automatically.",
                ]
                .concat(),
            }
            .fold();
            pretty_assertions::assert_eq!(
            output,
            [
                "220-2.0.0 this is a long message, a very very long message ... carriage return\r\n",
                "220 2.0.0  will be properly added automatically.\r\n",
            ]
            .concat()
        );
            for i in output.split("\r\n") {
                assert!(i.len() <= 78);
            }
        }

        #[test]
        fn two_line() {
            let output = Reply {
                code: ReplyCode::Enhanced {
                    code: 220,
                    enhanced: "2.0.0".to_string(),
                },
                text: [
                    "this is a long message, a very very long message ...",
                    " carriage return will be properly added automatically. Made by",
                    " vSMTP mail transfer agent\nCopyright (C) 2022 viridIT SAS",
                ]
                .concat(),
            }
            .fold();
            pretty_assertions::assert_eq!(
            output,
            [
                "220-2.0.0 this is a long message, a very very long message ... carriage return\r\n",
                "220-2.0.0  will be properly added automatically. Made by vSMTP mail transfer a\r\n",
                "220 2.0.0 gent\nCopyright (C) 2022 viridIT SAS\r\n",
            ]
            .concat()
        );
            for i in output.split("\r\n") {
                assert!(i.len() <= 78);
            }
        }

        #[test]
        fn ehlo_response() {
            let output = Reply {
                code: ReplyCode::Code { code: 250 },
                text: [
                    "testserver.com\r\n",
                    "AUTH PLAIN LOGIN CRAM-MD5\r\n",
                    "8BITMIME\r\n",
                    "SMTPUTF8\r\n",
                ]
                .concat(),
            }
            .fold();
            pretty_assertions::assert_eq!(
                output,
                [
                    "250-testserver.com\r\n",
                    "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
                    "250-8BITMIME\r\n",
                    "250 SMTPUTF8\r\n",
                ]
                .concat()
            );
            for i in output.split("\r\n") {
                assert!(i.len() <= 78);
            }
        }
    }

    mod parse {
        use crate::{Reply, ReplyCode};

        #[test]
        fn basic() {
            assert_eq!(
                Reply::parse_str("250 Ok").unwrap(),
                Reply {
                    code: ReplyCode::Code { code: 250 },
                    text: "Ok".to_string()
                }
            );
        }

        #[test]
        fn no_word() {
            assert_eq!(
                Reply::parse_str("250 ").unwrap(),
                Reply {
                    code: ReplyCode::Code { code: 250 },
                    text: "".to_string()
                }
            );
        }

        #[test]
        fn basic_enhanced() {
            assert_eq!(
                Reply::parse_str("501 5.1.7 Invalid address").unwrap(),
                Reply {
                    code: ReplyCode::Enhanced {
                        code: 501,
                        enhanced: "5.1.7".to_string()
                    },
                    text: "Invalid address".to_string()
                }
            );
        }
    }

    #[test]
    fn combine() {
        assert_eq!(
            Reply::combine(
                &Reply::new(
                    ReplyCode::Code { code: 454 },
                    "TLS not available due to temporary reason"
                ),
                &Reply::new(
                    ReplyCode::Code { code: 451 },
                    "Too many errors from the client"
                ),
            )
            .fold(),
            [
                "451-TLS not available due to temporary reason\r\n",
                "451 Too many errors from the client\r\n"
            ]
            .concat()
        );
    }
}
