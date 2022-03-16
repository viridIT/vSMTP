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
use vsmtp_common::libc_abstraction::if_nametoindex;

pub(super) fn serialize_version_req<S: serde::Serializer>(
    value: &semver::VersionReq,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.to_string(), serializer)
}

pub(super) fn deserialize_version_req<'de, D>(
    deserializer: D,
) -> Result<semver::VersionReq, D::Error>
where
    D: serde::Deserializer<'de>,
{
    semver::VersionReq::parse(&<String as serde::Deserialize>::deserialize(deserializer)?)
        .map_err(serde::de::Error::custom)
}

/// std::net::SocketAddr::parse does not support https://datatracker.ietf.org/doc/html/rfc4007#page-15
pub(super) fn deserialize_socket_addr<'de, D>(
    deserializer: D,
) -> Result<Vec<std::net::SocketAddr>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    fn ipv6_with_scope_id(input: &str) -> anyhow::Result<std::net::SocketAddr> {
        let (addr_ip_and_scope_name, colon_and_port) = input.split_at(
            input
                .rfind(':')
                .ok_or_else(|| anyhow::anyhow!("ipv6 port not provided"))?,
        );

        let (addr_ip, scope_name) = addr_ip_and_scope_name
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| anyhow::anyhow!("ipv6 not valid format"))?
            .split_once('%')
            .ok_or_else(|| anyhow::anyhow!("ipv6 no scope_id"))?;

        let mut socket_addr = format!("[{addr_ip}]{colon_and_port}")
            .parse::<std::net::SocketAddrV6>()
            .map_err(|e| anyhow::anyhow!("ipv6 parser produce error: '{e}'"))?;

        socket_addr.set_scope_id(if_nametoindex(scope_name)?);
        Ok(std::net::SocketAddr::V6(socket_addr))
    }

    <Vec<String> as serde::Deserialize>::deserialize(deserializer)?
        .into_iter()
        .map(|s| {
            <std::net::SocketAddr as std::str::FromStr>::from_str(&s)
                .or_else(|_| ipv6_with_scope_id(&s))
        })
        .collect::<anyhow::Result<Vec<std::net::SocketAddr>>>()
        .map_err(serde::de::Error::custom)
}

const ALL_PROTOCOL_VERSION: [ProtocolVersion; 2] = [
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
];

impl std::str::FromStr for ProtocolVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TLSv1.2" | "0x0303" => Ok(Self(rustls::ProtocolVersion::TLSv1_2)),
            "TLSv1.3" | "0x0304" => Ok(Self(rustls::ProtocolVersion::TLSv1_3)),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2",
            rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3",
            _ => todo!(),
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
