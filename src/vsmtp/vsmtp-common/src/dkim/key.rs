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

use super::{signature::ParseError, HashAlgorithm};

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Version {
    Dkim1,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum KeyType {
    Rsa,
}

impl Default for KeyType {
    fn default() -> Self {
        Self::Rsa
    }
}

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum ServiceType {
    #[strum(serialize = "*")]
    Wildcard,
    Email,
}

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum Flags {
    /// Verifiers MUST treat messages from Signers as unsigned email
    #[strum(serialize = "y")]
    Testing,
    /// the "i=" domain MUST NOT be a subdomain of "d="
    #[strum(serialize = "s")]
    SameDomain,
}

///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// tag "v="
    /// MUST be "DKIM1"
    pub version: Version,
    /// tag "h="
    pub acceptable_hash_algorithms: Vec<HashAlgorithm>,
    /// tag "k="
    pub r#type: KeyType,
    /// tag "n="
    /// a message to the administrator
    pub notes: Option<String>,
    /// tag "p="
    pub public_key: Vec<u8>,
    /// tag "s="
    /// default: "*"
    pub service_type: Vec<ServiceType>,
    /// tag "t="
    pub flags: Vec<Flags>,
}

impl std::str::FromStr for Key {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut version = Version::Dkim1;
        let mut acceptable_hash_algorithms =
            <HashAlgorithm as strum::IntoEnumIterator>::iter().collect::<Vec<_>>();
        let mut r#type = KeyType::default();
        let mut notes = None;
        let mut public_key = None;
        let mut service_type = vec![ServiceType::Wildcard];
        let mut flags = vec![];

        for i in s
            .split(';')
            .map(|tag| tag.split_whitespace().collect::<Vec<_>>().concat())
        {
            match i.split_once('=').ok_or(ParseError::SyntaxError {
                reason: "tag syntax is `{tag}={value}`".to_string(),
            })? {
                ("v", p_version) => {
                    version =
                        Version::from_str(p_version).map_err(|e| ParseError::SyntaxError {
                            reason: format!("when parsing `version`, got: `{e}`"),
                        })?;
                }
                ("h", p_acceptable_hash_algorithms) => {
                    acceptable_hash_algorithms = p_acceptable_hash_algorithms
                        .split(':')
                        // ignore unrecognized algorithms
                        .filter_map(|h| HashAlgorithm::from_str(h).ok())
                        .collect();
                }
                ("k", p_type) => {
                    r#type = KeyType::from_str(p_type).unwrap_or_default();
                }
                ("n", p_notes) => notes = Some(p_notes.to_string()),
                ("p", p_public_key) => {
                    public_key = Some(base64::decode(p_public_key).map_err(|e| {
                        ParseError::SyntaxError {
                            reason: format!("failed to pase `public_key`: got `{e}`"),
                        }
                    })?);
                }
                ("s", p_service_type) => {
                    service_type = p_service_type
                        .split(':')
                        // ignore unrecognized service type
                        .filter_map(|s| ServiceType::from_str(s).ok())
                        .collect();
                }
                ("t", p_flags) => {
                    flags = p_flags
                        .split(':')
                        // ignore unrecognized flags
                        .filter_map(|t| Flags::from_str(t).ok())
                        .collect();
                }
                // ignore unknown tag
                _ => continue,
            }
        }

        Ok(Self {
            version,
            acceptable_hash_algorithms,
            r#type,
            notes,
            public_key: public_key.ok_or(ParseError::MissingRequiredField {
                field: "public_key".to_string(),
            })?,
            service_type,
            flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::dkim::{
        key::{KeyType, ServiceType, Version},
        HashAlgorithm,
    };

    use super::Key;

    #[test]
    fn parse() {
        let txt= "v=DKIM1; h=sha256; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvxxZDZBe61KUSY/nQ09l9P9n4rmeb2Ol/Z2j7g33viWEfTCro0+Nyicz/vjTQZv+cq5Wla+ADyXkdSGJ0OFp9SrUu9tGeDhil2UEPsHHdnf3AaarX3hyY8Ne5X5EOnJ5WY3QSpTL+eVUtSTt5DbsDqfShzxbc/BsKb5sfHuGJxcKuCyFVqCyhpSKT4kdpzZ5FLLrEiyvJGYUfq7qvqPB+A/wx1TIO5YONWWH2mqy3zviLx70u06wnxwyvGve2HMKeMvDm1HGibZShJnOIRzJuZ9BFYffm8iGisYFocxp7daiJgbpMtqYY/TB8ZvGajv/ZqITrbRp+qpfK9Bpdk8qXwIDAQAB";

        assert_eq!(
            <Key as std::str::FromStr>::from_str(txt).unwrap(),
            Key {
                version: Version::Dkim1,
                acceptable_hash_algorithms: vec![HashAlgorithm::Sha256],
                r#type: KeyType::Rsa,
                notes: None,
                public_key: base64::decode("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvxxZDZBe61KUSY/nQ09l9P9n4rmeb2Ol/Z2j7g33viWEfTCro0+Nyicz/vjTQZv+cq5Wla+ADyXkdSGJ0OFp9SrUu9tGeDhil2UEPsHHdnf3AaarX3hyY8Ne5X5EOnJ5WY3QSpTL+eVUtSTt5DbsDqfShzxbc/BsKb5sfHuGJxcKuCyFVqCyhpSKT4kdpzZ5FLLrEiyvJGYUfq7qvqPB+A/wx1TIO5YONWWH2mqy3zviLx70u06wnxwyvGve2HMKeMvDm1HGibZShJnOIRzJuZ9BFYffm8iGisYFocxp7daiJgbpMtqYY/TB8ZvGajv/ZqITrbRp+qpfK9Bpdk8qXwIDAQAB").unwrap(),
                service_type: vec![ServiceType::Wildcard],
                flags: vec![]
            }
        );
    }
}
