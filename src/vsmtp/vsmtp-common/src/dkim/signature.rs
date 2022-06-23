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

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing required field: `{field}`")]
    MissingRequiredField { field: String },
    #[error("syntax error: `{reason}`")]
    SyntaxError { reason: String },
    #[error("invalid argument: `{reason}`")]
    InvalidArgument { reason: String },
}

#[derive(Debug, PartialEq, Eq, strum::EnumString, strum::Display)]
enum SigningAlgorithm {
    #[strum(serialize = "rsa-sha1")]
    RsaSha1,
    #[strum(serialize = "rsa-sha256")]
    RsaSha256,
}

#[derive(Debug, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
enum CanonicalizationAlgorithm {
    Simple,
    Relaxed,
}

#[derive(Debug, PartialEq, Eq)]
struct Canonicalization {
    header: CanonicalizationAlgorithm,
    body: CanonicalizationAlgorithm,
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

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct QueryMethod {
    r#type: String,
    options: String,
}

impl Default for QueryMethod {
    fn default() -> Self {
        Self {
            r#type: "dns".to_string(),
            options: "txt".to_string(),
        }
    }
}

impl std::str::FromStr for QueryMethod {
    type Err = ParseError;

    // NOTE: currently "dns/txt" is the only format supported (by signers and verifiers)
    // but others might be added in the future
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
#[derive(Debug, PartialEq, Eq)]
pub struct Signature {
    version: usize,
    signing_algorithm: SigningAlgorithm,
    /// Signing Domain Identifier (SDID)
    sdid: String,
    selector: String,
    canonicalization: Canonicalization,
    query_method: Vec<QueryMethod>,
    /// Agent or User Identifier (AUID)
    auid: String,
    signature_timestamp: Option<std::time::Duration>,
    expire_time: Option<std::time::Duration>,
    body_length: Option<usize>,
    headers_field: Vec<String>,
    copy_header_fields: Option<Vec<(String, String)>>,
    body_hash: Vec<u8>,
    signature: Vec<u8>,
}

impl std::str::FromStr for Signature {
    type Err = ParseError;

    #[allow(clippy::too_many_lines)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

        for i in s
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
                    body_hash =
                        Some(
                            base64::decode(p_body_hash).map_err(|e| ParseError::SyntaxError {
                                reason: format!("failed to pase `body_hash`: got `{e}`"),
                            })?,
                        );
                }
                ("b", p_signature) => {
                    signature =
                        Some(
                            base64::decode(p_signature).map_err(|e| ParseError::SyntaxError {
                                reason: format!("failed to pase `signature`: got `{e}`"),
                            })?,
                        );
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Canonicalization, CanonicalizationAlgorithm, QueryMethod, Signature, SigningAlgorithm,
    };

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

        let sign =
            <Signature as std::str::FromStr>::from_str(&signature["DKIM-Signature: ".len()..])
                .unwrap();
        assert_eq!(
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
                body_hash: base64::decode("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI").unwrap(),
                signature: base64::decode(
                    "dzdVyOfAKCdLXdJOc9G2q8LoXSlEniSbav+yuU4zGeeruD00lszZVoG4ZHRNiYzR"
                )
                .unwrap()
            }
        );
        println!("{sign:#?}");
    }
}
