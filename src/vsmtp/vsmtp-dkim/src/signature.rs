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

use vsmtp_common::RawBody;

use super::{Canonicalization, SigningAlgorithm};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing required field: `{field}`")]
    MissingRequiredField { field: String },
    #[error("syntax error: `{reason}`")]
    SyntaxError { reason: String },
    #[error("invalid argument: `{reason}`")]
    InvalidArgument { reason: String },
}

// NOTE: currently "dns/txt" is the only format supported (by signers and verifiers)
// but others might be added in the future
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct QueryMethod {
    // r#type: String,
    // options: String,
}

impl std::fmt::Display for QueryMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dns/txt")
    }
}

impl std::str::FromStr for QueryMethod {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "dns/txt" {
            Ok(Self::default())
        } else {
            Err(ParseError::InvalidArgument {
                reason: format!("`{s}` is not a valid query method"),
            })
        }
    }
}

/// Representation of the "DKIM-Signature" header
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Signature {
    /// tag "v="
    pub version: usize,
    /// tag "a="
    pub signing_algorithm: SigningAlgorithm,
    /// Signing Domain Identifier (SDID)
    /// tag "d="
    pub sdid: String,
    /// tag "s="
    pub selector: String,
    /// tag "c="
    pub canonicalization: Canonicalization,
    /// tag "q="
    pub query_method: Vec<QueryMethod>,
    /// Agent or User Identifier (AUID)
    /// tag "i=", or "@d" is "i" is missing
    pub auid: String,
    /// tag "t="
    pub signature_timestamp: Option<std::time::Duration>,
    /// tag "x="
    pub expire_time: Option<std::time::Duration>,
    /// tag "l="
    pub body_length: Option<usize>,
    /// tag "h="
    pub headers_field: Vec<String>,
    /// tag "z="
    pub copy_header_fields: Option<Vec<(String, String)>>,
    /// tag "bh="
    pub body_hash: String,
    /// tag "b="
    pub signature: String,
    pub(crate) raw: String,
}

impl Signature {
    ///
    #[must_use]
    pub fn has_expired(&self, epsilon: u64) -> bool {
        match self.expire_time {
            Some(expire_time) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards");

                // `now - x` or `x - now`, to get +/-
                match now
                    .as_secs()
                    .checked_sub(expire_time.as_secs())
                    .or_else(|| expire_time.as_secs().checked_sub(now.as_secs()))
                {
                    Some(diff) => diff > epsilon,
                    None => false,
                }
            }
            // expiration date is undefined
            None => false,
        }
    }

    ///
    #[must_use]
    pub fn get_dns_query(&self) -> String {
        format!(
            "{selector}._domainkey.{sdid}",
            selector = self.selector,
            sdid = self.sdid
        )
    }

    ///
    #[must_use]
    pub fn get_signature_value(&self) -> String {
        self.raw["DKIM-Signature:".len()..].to_string()
    }

    fn signature_without_headers(&self) -> String {
        let mut out = self.raw.to_string();
        if self.signature.is_empty() {
            return out;
        }

        let begin_hash = self.raw.find("b=").unwrap() + 2;
        let end_hash = &self.signature[self.signature.len() - 4..self.signature.len()];

        out.replace_range(
            begin_hash..begin_hash + self.raw[begin_hash..].find(end_hash).unwrap() + 4,
            "",
        );
        out
    }

    ///
    #[must_use]
    pub fn get_header_hash(&self, message: &RawBody) -> Vec<u8> {
        let mut last_index = std::collections::HashMap::<&str, usize>::new();

        let headers = message.headers();

        let mut output = vec![];
        for header in &self.headers_field {
            let idx = last_index
                .get(header.as_str())
                .map_or(headers.len(), |x| *x);

            if let Some((pos, (key, value))) = headers[..idx]
                .iter()
                .enumerate()
                .rfind(|(_, (key, _))| key.to_lowercase() == header.to_lowercase())
            {
                last_index
                    .entry(key.as_str())
                    .and_modify(|v| *v = pos)
                    .or_insert(pos);
                output.push(format!("{key}:{value}\r\n"));
            }
        }

        let mut output = self.canonicalization.header.canonicalize_headers(&output);

        output.push_str(
            &self
                .canonicalization
                .header
                .canonicalize_header(&self.signature_without_headers()),
        );

        self.signing_algorithm.hash(output)
    }
}

impl std::str::FromStr for Signature {
    type Err = ParseError;

    #[allow(clippy::too_many_lines)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.to_lowercase().starts_with("dkim-signature:") {
            return Err(ParseError::InvalidArgument {
                reason: "not a dkim-signature header".to_string(),
            });
        }

        let mut version = None;
        let mut signing_algorithm = None;
        let mut sdid = None;
        let mut selector = None;
        let mut canonicalization = Canonicalization::default();
        let mut query_method = vec![QueryMethod::default()];
        let mut auid = None;
        let mut signature_timestamp = None;
        let mut expire_time = None;
        let mut body_length = None;
        let mut headers_field = None;
        let mut copy_header_fields = None;
        let mut body_hash = None;
        let mut signature = None;

        for i in s["dkim-signature:".len()..]
            .split(';')
            .map(|tag| tag.split_whitespace().collect::<Vec<_>>().concat())
        {
            match i.split_once('=').ok_or(ParseError::SyntaxError {
                reason: "tag syntax is `{tag}={value}`".to_string(),
            })? {
                ("v", p_version) => {
                    version =
                        Some(
                            p_version
                                .parse::<usize>()
                                .map_err(|e| ParseError::SyntaxError {
                                    reason: format!("when parsing `version`, got: `{e}`"),
                                })?,
                        );
                }
                ("a", p_signing_algorithm) => {
                    signing_algorithm = Some(
                        SigningAlgorithm::from_str(p_signing_algorithm).map_err(|e| {
                            ParseError::SyntaxError {
                                reason: format!("when parsing `signing_algorithm`, got: `{e}`"),
                            }
                        })?,
                    );
                }
                ("d", p_sdid) => sdid = Some(p_sdid.to_string()),
                ("s", p_selector) => selector = Some(p_selector.to_string()),
                ("c", p_canonicalization) => {
                    canonicalization =
                        Canonicalization::from_str(p_canonicalization).map_err(|e| {
                            ParseError::SyntaxError {
                                reason: format!("when parsing `canonicalization`, got: `{e}`"),
                            }
                        })?;
                }
                ("q", p_query_method) => {
                    query_method = p_query_method
                        .split(':')
                        .map(QueryMethod::from_str)
                        .collect::<Result<Vec<_>, ParseError>>()?;
                }
                ("i", p_auid) => auid = Some(p_auid.to_string()),
                ("t", p_signature_timestamp) => {
                    signature_timestamp = Some(std::time::Duration::from_secs(
                        p_signature_timestamp.parse::<u64>().map_err(|e| {
                            ParseError::SyntaxError {
                                reason: format!("when parsing `signature_timestamp`, got: `{e}`"),
                            }
                        })?,
                    ));
                }
                ("x", p_expire_time) => {
                    expire_time = Some(std::time::Duration::from_secs(
                        p_expire_time
                            .parse::<u64>()
                            .map_err(|e| ParseError::SyntaxError {
                                reason: format!("when parsing `expire_time`, got: `{e}`"),
                            })?,
                    ));
                }
                ("l", p_body_length) => {
                    body_length = Some(p_body_length.parse::<usize>().map_err(|e| {
                        ParseError::SyntaxError {
                            reason: format!("when parsing `body_length`, got: `{e}`"),
                        }
                    })?);
                }
                ("h", p_headers_field) => {
                    headers_field = Some(
                        p_headers_field
                            .split(':')
                            .map(str::to_string)
                            .collect::<Vec<_>>(),
                    );
                }
                ("z", p_copy_header_fields) => {
                    copy_header_fields = Some(
                        p_copy_header_fields
                            .split('|')
                            .map(|s| match s.split_once(':') {
                                Some((k, v)) => Ok((k.to_string(), v.to_string())),
                                None => Err(ParseError::SyntaxError {
                                    reason: "tag syntax is `{header}={value}`".to_string(),
                                }),
                            })
                            .collect::<Result<Vec<_>, ParseError>>()?,
                    );
                }
                ("bh", p_body_hash) => {
                    base64::decode(p_body_hash).map_err(|e| ParseError::SyntaxError {
                        reason: format!("failed to pase `body_hash`: got `{e}`"),
                    })?;

                    body_hash = Some(p_body_hash.to_string());
                }
                ("b", p_signature) => {
                    base64::decode(p_signature).map_err(|e| ParseError::SyntaxError {
                        reason: format!("failed to pase `signature`: got `{e}`"),
                    })?;

                    signature = Some(p_signature.to_string());
                }
                // unknown tags are ignored
                _ => continue,
            }
        }

        let sdid = sdid.ok_or(ParseError::MissingRequiredField {
            field: "sdid".to_string(),
        })?;

        Ok(Signature {
            version: version.ok_or(ParseError::MissingRequiredField {
                field: "version".to_string(),
            })?,
            signing_algorithm: signing_algorithm.ok_or(ParseError::MissingRequiredField {
                field: "signing_algorithm".to_string(),
            })?,
            sdid: sdid.clone(),
            selector: selector.ok_or(ParseError::MissingRequiredField {
                field: "selector".to_string(),
            })?,
            canonicalization,
            query_method,
            auid: {
                let auid = auid.unwrap_or_else(|| format!("@{sdid}"));
                if !auid.ends_with(&sdid) {
                    return Err(ParseError::InvalidArgument {
                        reason: format!(
                            "`auid` ({auid}) must be a subdomain or the same as `sdid` ({sdid})"
                        ),
                    });
                }

                auid
            },
            signature_timestamp,
            expire_time,
            body_length,
            headers_field: {
                let headers_field = headers_field.ok_or(ParseError::MissingRequiredField {
                    field: "headers_field".to_string(),
                })?;
                if headers_field.is_empty() {
                    return Err(ParseError::InvalidArgument {
                        reason: "`headers_field` must not be empty".to_string(),
                    });
                } else if headers_field
                    .iter()
                    .map(|s| s.to_lowercase())
                    .any(|s| &s == "dkim-signature")
                {
                    return Err(ParseError::InvalidArgument {
                        reason: "`headers_field` must not contains `DKIM-Signature`".to_string(),
                    });
                }
                headers_field
            },
            copy_header_fields,
            body_hash: body_hash.ok_or(ParseError::MissingRequiredField {
                field: "body_hash".to_string(),
            })?,
            signature: signature.ok_or(ParseError::MissingRequiredField {
                field: "signature".to_string(),
            })?,
            raw: s.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Canonicalization, QueryMethod, Signature, SigningAlgorithm};
    use crate::CanonicalizationAlgorithm;

    #[test]
    fn from_str_wikipedia() {
        let signature = [
            "DKIM-Signature: v=1; a=rsa-sha256; d=example.net; s=brisbane;",
            "    c=relaxed/simple; q=dns/txt; i=foo@eng.example.net;",
            "    t=1117574938; x=1118006938; l=200;",
            "    h=from:to:subject:date:keywords:keywords;",
            "    z=From:foo@eng.example.net|To:joe@example.com|",
            "      Subject:demo=20run|Date:July=205,=202005=203:44:08=20PM=20-0700;",
            "    bh=MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=;",
            "    b=dzdVyOfAKCdLXdJOc9G2q8LoXSlEniSbav+yuU4zGeeruD00lszZ",
            "             VoG4ZHRNiYzR",
        ]
        .concat();

        let sign = <Signature as std::str::FromStr>::from_str(&signature).unwrap();
        pretty_assertions::assert_eq!(
            sign,
            Signature {
                version: 1,
                signing_algorithm: SigningAlgorithm::RsaSha256,
                sdid: "example.net".to_string(),
                selector: "brisbane".to_string(),
                canonicalization: Canonicalization {
                    header: CanonicalizationAlgorithm::Relaxed,
                    body: CanonicalizationAlgorithm::Simple
                },
                query_method: vec![QueryMethod::default()],
                auid: "foo@eng.example.net".to_string(),
                signature_timestamp: Some(std::time::Duration::from_secs(1_117_574_938)),
                expire_time: Some(std::time::Duration::from_secs(1_118_006_938)),
                body_length: Some(200),
                headers_field: ["from", "to", "subject", "date", "keywords", "keywords",]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                copy_header_fields: Some(
                    [
                        ("From", "foo@eng.example.net"),
                        ("To", "joe@example.com"),
                        ("Subject", "demo=20run"),
                        ("Date", "July=205,=202005=203:44:08=20PM=20-0700"),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
                ),
                body_hash: "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=".to_string(),
                signature: "dzdVyOfAKCdLXdJOc9G2q8LoXSlEniSbav+yuU4zGeeruD00lszZVoG4ZHRNiYzR"
                    .to_string(),
                raw: signature
            }
        );
        assert!(sign.has_expired(100));
        assert!(!sign.has_expired(1_000_000_000));
        println!("{sign:#?}");

        println!("{sign}");
    }
}
