use crate::smtp::code::SMTPReplyCode;

/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use super::server_config::{ProtocolVersion, ProtocolVersionRequirement};

pub(super) fn ordered_map<S>(
    value: &std::collections::HashMap<SMTPReplyCode, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde::Serialize::serialize(
        &value.iter().collect::<std::collections::BTreeMap<_, _>>(),
        serializer,
    )
}

const ALL_PROTOCOL_VERSION: [ProtocolVersion; 6] = [
    ProtocolVersion(rustls::ProtocolVersion::SSLv2),
    ProtocolVersion(rustls::ProtocolVersion::SSLv3),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_0),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
];

impl std::str::FromStr for ProtocolVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SSLv2" | "0x0200" => Ok(ProtocolVersion(rustls::ProtocolVersion::SSLv2)),
            "SSLv3" | "0x0300" => Ok(ProtocolVersion(rustls::ProtocolVersion::SSLv3)),
            "TLSv1.0" | "0x0301" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)),
            "TLSv1.1" | "0x0302" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)),
            "TLSv1.2" | "0x0303" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)),
            "TLSv1.3" | "0x0304" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            rustls::ProtocolVersion::SSLv2 => "SSLv2",
            rustls::ProtocolVersion::SSLv3 => "SSLv3",
            rustls::ProtocolVersion::TLSv1_0 => "SSLv1.0",
            rustls::ProtocolVersion::TLSv1_1 => "SSLv1.1",
            rustls::ProtocolVersion::TLSv1_2 => "SSLv1.2",
            rustls::ProtocolVersion::TLSv1_3 => "SSLv1.3",
            rustls::ProtocolVersion::Unknown(_) => todo!(),
        })
    }
}

impl serde::Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = \"SSLv2\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv2)]);
/// let s = toml::from_str::<S>("v = \"0x0200\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv2)]);
///
/// let s = toml::from_str::<S>("v = \"SSLv3\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv3)]);
/// let s = toml::from_str::<S>("v = \"0x0300\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::SSLv3)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.0\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)]);
/// let s = toml::from_str::<S>("v = \"0x0301\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.1\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)]);
/// let s = toml::from_str::<S>("v = \"0x0302\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.2\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]);
/// let s = toml::from_str::<S>("v = \"0x0303\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]);
///
/// let s = toml::from_str::<S>("v = \"TLSv1.3\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]);
/// let s = toml::from_str::<S>("v = \"0x0304\"").unwrap();
/// assert_eq!(s.v.0, vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]);
/// ```
///
/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = [\"TLSv1.1\", \"TLSv1.2\", \"TLSv1.3\"]").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
/// ```
///
/// ```
/// use vsmtp::config::server_config::ProtocolVersion;
/// use vsmtp::config::server_config::ProtocolVersionRequirement;
///
/// #[derive(Debug, serde::Deserialize)]
/// struct S {
///     v: ProtocolVersionRequirement,
/// }
///
/// let s = toml::from_str::<S>("v = \"^TLSv1.1\"").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
///
/// let s = toml::from_str::<S>("v = \">=SSLv3\"").unwrap();
/// assert_eq!(s.v.0, vec![
///     ProtocolVersion(rustls::ProtocolVersion::SSLv3),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_0),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
///     ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
/// ]);
///
/// ```
impl<'de> serde::Deserialize<'de> for ProtocolVersionRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ProtocolVersionVisitor;

        impl<'de> serde::de::Visitor<'de> for ProtocolVersionVisitor {
            type Value = ProtocolVersionRequirement;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("[...]")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.strip_prefix(">=").or_else(|| v.strip_prefix('^')) {
                    Some(v) => {
                        let min_value = <ProtocolVersion as std::str::FromStr>::from_str(v)
                            .map_err(|_| serde::de::Error::custom("invalid"))?;
                        let mut min_value_idx = 0;
                        for (idx, i) in ALL_PROTOCOL_VERSION.iter().enumerate() {
                            if *i == min_value {
                                min_value_idx = idx;
                            }
                        }

                        Ok(ProtocolVersionRequirement(
                            ALL_PROTOCOL_VERSION[min_value_idx..].to_vec(),
                        ))
                    }
                    None => Ok(ProtocolVersionRequirement(vec![
                        <ProtocolVersion as std::str::FromStr>::from_str(v)
                            .map_err(|_| serde::de::Error::custom("invalid"))?,
                    ])),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut v = Vec::<Result<ProtocolVersion, A::Error>>::new();
                while let Some(i) = seq.next_element::<&str>()? {
                    v.push(
                        <ProtocolVersion as std::str::FromStr>::from_str(i)
                            .map_err(|_| serde::de::Error::custom("invalid")),
                    );
                }

                Ok(ProtocolVersionRequirement(v.into_iter().collect::<Result<
                    Vec<ProtocolVersion>,
                    A::Error,
                >>(
                )?))
            }
        }

        deserializer.deserialize_any(ProtocolVersionVisitor)
    }
}
