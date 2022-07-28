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

pub mod parsing;
use crate::api::SharedObject;
use vsmtp_common::{
    re::{addr, anyhow, log, strum},
    Address, Reply, ReplyCode,
};

const FILE_CAPACITY: usize = 20;

/// Objects are rust's representation of rule engine variables.
/// multiple types are supported.
#[derive(Debug, Clone, strum::AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum Object {
    /// ip v4 address. (a.b.c.d)
    #[strum(serialize = "ip4")]
    Ip4(std::net::Ipv4Addr),
    /// ip v6 address. (x:x:x:x:x:x:x:x)
    #[strum(serialize = "ip6")]
    Ip6(std::net::Ipv6Addr),
    /// an ip v4 range. (a.b.c.d/range)
    #[strum(serialize = "rg4")]
    Rg4(iprange::IpRange<ipnet::Ipv4Net>),
    /// an ip v6 range. (x:x:x:x:x:x:x:x/range)
    #[strum(serialize = "rg6")]
    Rg6(iprange::IpRange<ipnet::Ipv6Net>),
    /// an email address (jones@foo.com)
    #[strum(serialize = "address")]
    Address(Address),
    /// a valid fully qualified domain name (foo.com)
    #[strum(serialize = "fqdn")]
    Fqdn(String),
    /// a regex (^[a-z0-9.]+@foo.com$)
    #[strum(serialize = "regex")]
    Regex(regex::Regex),
    /// the content of a file.
    #[strum(serialize = "file")]
    File(Vec<Object>),
    /// a group of objects declared inline.
    #[strum(serialize = "group")]
    Group(Vec<SharedObject>),
    /// a user.
    #[strum(serialize = "identifier")]
    Identifier(String),
    /// a simple string.
    #[strum(serialize = "string")]
    Str(String),
    /// a custom smtp reply code.
    #[strum(serialize = "code")]
    Code(Reply),
}

impl Object {
    /// get a specific value from a rhai map and convert it to a specific type.
    /// returns an error if the cast failed.
    pub(crate) fn value<S, T>(
        map: &std::collections::BTreeMap<S, rhai::Dynamic>,
        key: &str,
    ) -> anyhow::Result<T>
    where
        S: std::str::FromStr + std::cmp::Ord,
        T: Clone + 'static,
    {
        match map.get(
            &S::from_str(key)
                .map_err(|_| anyhow::anyhow!("failed to get {key} key from an object"))?,
        ) {
            Some(value) => value.clone().try_cast::<T>().ok_or_else(|| {
                anyhow::anyhow!("{} is not of type {}.", key, std::any::type_name::<T>())
            }),
            None => anyhow::bail!("'{}' key not found in object.", key),
        }
    }

    /// create an object from a raw rhai Map data structure.
    /// this map must have the "value" and "type" keys to be parsed
    /// successfully.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn from_map<S>(
        map: &std::collections::BTreeMap<S, rhai::Dynamic>,
    ) -> anyhow::Result<Self>
    where
        S: std::fmt::Debug + std::str::FromStr + std::cmp::Ord + 'static,
    {
        let t = Self::value::<S, String>(map, "type")?;

        match t.as_str() {
            "ip4" => Ok(Self::Ip4(
                <std::net::Ipv4Addr as std::str::FromStr>::from_str(&Self::value::<S, String>(
                    map, "value",
                )?)?,
            )),

            "ip6" => Ok(Self::Ip6(
                <std::net::Ipv6Addr as std::str::FromStr>::from_str(&Self::value::<S, String>(
                    map, "value",
                )?)?,
            )),

            "rg4" => Ok(Self::Rg4(
                [Self::value::<S, String>(map, "value")?.parse::<ipnet::Ipv4Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "rg6" => Ok(Self::Rg6(
                [Self::value::<S, String>(map, "value")?.parse::<ipnet::Ipv6Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "fqdn" => {
                let value = Self::value::<S, String>(map, "value")?;
                match addr::parse_domain_name(&value) {
                    Ok(domain) => Ok(Self::Fqdn(domain.to_string())),
                    Err(_) => anyhow::bail!("'{}' is not a valid fqdn.", value),
                }
            }

            "address" => {
                let value = Self::value::<S, String>(map, "value")?;
                Ok(Self::Address(Address::try_from(value)?))
            }

            "identifier" => Ok(Self::Identifier(Self::value::<S, String>(map, "value")?)),

            "string" => Ok(Self::Str(Self::value::<S, String>(map, "value")?)),

            "regex" => Ok(Self::Regex(<regex::Regex as std::str::FromStr>::from_str(
                &Self::value::<S, String>(map, "value")?,
            )?)),

            // the file object as an extra "content_type" parameter.
            "file" => {
                let path = Self::value::<S, String>(map, "value")?;

                if !std::path::PathBuf::from(&path).is_absolute() {
                    anyhow::bail!("a file object path must be absolute: '{}' is invalid", path);
                }

                let content_type = Self::value::<S, String>(map, "content_type")?;
                let reader = std::io::BufReader::new(std::fs::File::open(&path)?);
                let mut content = Vec::with_capacity(FILE_CAPACITY);

                for line in std::io::BufRead::lines(reader) {
                    match line {
                        Ok(line) => match content_type.as_str() {
                            "ip4" => content.push(Self::Ip4(
                                <std::net::Ipv4Addr as std::str::FromStr>::from_str(&line)?,
                            )),
                            "ip6" => content.push(Self::Ip6(
                                <std::net::Ipv6Addr as std::str::FromStr>::from_str(&line)?,
                            )),
                            "fqdn" => match addr::parse_domain_name(&line) {
                                Ok(domain) => content.push(Self::Fqdn(domain.to_string())),
                                Err(_) => anyhow::bail!("'{}' is not a valid fqdn.", path),
                            },
                            "address" => {
                                content.push(Self::Address(Address::try_from(line)?));
                            }
                            "string" => content.push(Self::Str(line)),
                            "identifier" => content.push(Self::Identifier(line)),
                            "regex" => content.push(Self::Regex(
                                <regex::Regex as std::str::FromStr>::from_str(&line)?,
                            )),
                            _ => {}
                        },
                        Err(error) => log::error!("couldn't read line in '{}': {}", path, error),
                    };
                }

                Ok(Self::File(content))
            }

            "group" => {
                let mut group = vec![];
                let elements = Self::value::<S, rhai::Array>(map, "value")?;
                let name = Self::value::<S, String>(map, "name")?;

                for element in elements {
                    group.push(
                        element
                            .clone()
                            .try_cast::<std::sync::Arc<Self>>()
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "the element '{:?}' inside the '{}' group is not an object",
                                    element,
                                    name
                                )
                            })?,
                    );
                }

                Ok(Self::Group(group))
            }

            "code" => {
                if let Ok(code) = Self::value::<S, String>(map, "value") {
                    Ok(Self::Code(Reply::parse_str(&code)?))
                } else {
                    let code = u16::try_from(Self::value::<S, i64>(map, "code")?)?;

                    Ok(Self::Code(Reply::new(
                        if let Ok(enhanced) = Self::value::<S, String>(map, "enhanced") {
                            ReplyCode::Enhanced { code, enhanced }
                        } else {
                            ReplyCode::Code { code }
                        },
                        Self::value::<S, String>(map, "text")?,
                    )))
                }
            }
            _ => anyhow::bail!("'{}' is an unknown object type.", t),
        }
    }

    /// check if the `other` object is contained in this object.
    ///
    /// # Errors
    /// * `self` cannot contain `other`.
    pub fn contains(&self, other: &Self) -> anyhow::Result<bool> {
        match (self, other) {
            (Object::Group(group), other) => Ok(group.iter().any(|element| *other == **element)),
            (Object::File(file), other) => Ok(file.iter().any(|element| *other == *element)),
            (Object::Rg4(rg4), Object::Ip4(ip4)) => Ok(rg4.contains(ip4)),
            (Object::Rg6(rg6), Object::Ip6(ip6)) => Ok(rg6.contains(ip6)),
            #[allow(clippy::unnecessary_to_owned)]
            (Object::Regex(regex), other) => Ok(regex.find(&other.to_string()).is_some()),
            (Object::Address(addr), Object::Identifier(identifier)) => {
                Ok(addr.local_part() == identifier.as_str())
            }
            (Object::Address(addr), Object::Fqdn(fqdn)) => Ok(addr.domain() == fqdn.as_str()),
            _ => {
                anyhow::bail!(
                    "cannot look for a '{}' object in a '{}' object",
                    other.as_ref(),
                    self.as_ref()
                )
            }
        }
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ip4(l0), Self::Ip4(r0)) => l0 == r0,
            (Self::Ip6(l0), Self::Ip6(r0)) => l0 == r0,
            (Self::Rg4(l0), Self::Rg4(r0)) => l0 == r0,
            (Self::Rg6(l0), Self::Rg6(r0)) => l0 == r0,
            (Self::Address(l0), Self::Address(r0)) => l0 == r0,
            (Self::Fqdn(l0), Self::Fqdn(r0))
            | (Self::Identifier(l0), Self::Identifier(r0))
            | (Self::Str(l0), Self::Str(r0)) => l0 == r0,
            (Self::File(l0), Self::File(r0)) => l0 == r0,
            (Self::Group(l0), Self::Group(r0)) => l0 == r0,
            (Self::Regex(r0), Self::Regex(l0)) => r0.as_str() == l0.as_str(),
            (Self::Code(r0), Self::Code(l0)) => r0 == l0,
            (Self::Str(string), any) | (any, Self::Str(string)) => *string == any.to_string(),
            _ => false,
        }
    }
}

impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::Ip4(ip) => write!(f, "{ip}"),
            Object::Ip6(ip) => write!(f, "{ip}"),
            Object::Rg4(range) => write!(f, "{range:?}"),
            Object::Rg6(range) => write!(f, "{range:?}"),
            Object::Address(addr) => write!(f, "{addr}"),
            Object::Fqdn(fqdn) => write!(f, "{fqdn}"),
            Object::Regex(regex) => write!(f, "{regex}"),
            Object::File(file) => write!(f, "{file:?}"),
            Object::Group(group) => write!(f, "{group:?}"),
            Object::Identifier(string) | Object::Str(string) => write!(f, "{string}"),
            Object::Code(reply) => write!(f, "{} {}", reply.code(), reply.text()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Object;
    use std::net::Ipv4Addr;
    use vsmtp_common::{addr, Reply, ReplyCode};

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_from() {
        let ip4 = Object::from_map(&rhai::Map::from_iter([
            ("name".into(), rhai::Dynamic::from("ip4".to_string())),
            ("type".into(), rhai::Dynamic::from("ip4".to_string())),
            ("value".into(), rhai::Dynamic::from("127.0.0.1".to_string())),
        ]))
        .unwrap();
        assert_eq!(ip4, Object::Ip4(Ipv4Addr::new(127, 0, 0, 1)));

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("ip6".into(), rhai::Dynamic::from("ip6".to_string())),
                ("type".into(), rhai::Dynamic::from("ip6".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from("2001:0db8:0000:85a3:0000:0000:ac1f:8001".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Ip6("2001:0db8:0000:85a3:0000:0000:ac1f:8001".parse().unwrap())
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("rg4".to_string())),
                ("type".into(), rhai::Dynamic::from("rg4".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from("192.168.0.0/24".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Rg4(
                ["192.168.0.0/24"]
                    .into_iter()
                    .map(|x| x.parse().unwrap())
                    .collect()
            )
        );

        let rg6 = Object::from_map(&rhai::Map::from_iter([
            ("name".into(), rhai::Dynamic::from("rg6".to_string())),
            ("type".into(), rhai::Dynamic::from("rg6".to_string())),
            (
                "value".into(),
                rhai::Dynamic::from("2001:db8:1234::/48".to_string()),
            ),
        ]))
        .unwrap();
        assert_eq!(
            rg6,
            Object::Rg6(
                ["2001:db8:1234::/48"]
                    .into_iter()
                    .map(|x| x.parse().unwrap())
                    .collect()
            )
        );

        let fqdn = Object::from_map(&rhai::Map::from_iter([
            ("name".into(), rhai::Dynamic::from("fqdn".to_string())),
            ("type".into(), rhai::Dynamic::from("fqdn".to_string())),
            (
                "value".into(),
                rhai::Dynamic::from("example.com".to_string()),
            ),
        ]))
        .unwrap();
        assert_eq!(fqdn, Object::Fqdn("example.com".to_string()));

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("address".to_string())),
                ("type".into(), rhai::Dynamic::from("address".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from("john@doe.com".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Address(addr!("john@doe.com"))
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("identifier".to_string())),
                ("type".into(), rhai::Dynamic::from("identifier".to_string())),
                ("value".into(), rhai::Dynamic::from("john".to_string())),
            ]))
            .unwrap(),
            Object::Identifier("john".to_string())
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("string".to_string())),
                ("type".into(), rhai::Dynamic::from("string".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from("a text string".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Str("a text string".to_string())
        );

        assert_eq!(
            format!(
                "{}",
                Object::from_map(&rhai::Map::from_iter([
                    ("name".into(), rhai::Dynamic::from("regex".to_string())),
                    ("type".into(), rhai::Dynamic::from("regex".to_string())),
                    (
                        "value".into(),
                        rhai::Dynamic::from("^[a-z0-9.]+.com$".to_string()),
                    ),
                ]))
                .unwrap()
            ),
            regex::Regex::new("^[a-z0-9.]+.com$").unwrap().as_str()
        );

        // TODO: test all possible content types.
        // assert_eq!(
        //     Object::from_map(&rhai::Map::from_iter([
        //         ("name".into(), rhai::Dynamic::from("file".to_string())),
        //         ("type".into(), rhai::Dynamic::from("file".to_string())),
        //         (
        //             "content_type".into(),
        //             rhai::Dynamic::from("address".to_string()),
        //         ),
        //         (
        //             "value".into(),
        //             rhai::Dynamic::from("./src/tests/types/address/whitelist.txt".to_string()),
        //         ),
        //     ]))
        //     .unwrap(),
        //     Object::File(vec![
        //         Object::Address(addr!("foo@bar.net")),
        //         Object::Address(addr!("nested@address.com")),
        //         Object::Address(addr!("john@doe.com"))
        //     ])
        // );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("group".to_string())),
                ("type".into(), rhai::Dynamic::from("group".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from(rhai::Array::from_iter([
                        rhai::Dynamic::from(std::sync::Arc::new(ip4)),
                        rhai::Dynamic::from(std::sync::Arc::new(rg6)),
                        rhai::Dynamic::from(std::sync::Arc::new(fqdn)),
                    ])),
                ),
            ]))
            .unwrap(),
            Object::Group(vec![
                std::sync::Arc::new(Object::Ip4(Ipv4Addr::new(127, 0, 0, 1))),
                std::sync::Arc::new(Object::Rg6(
                    ["2001:db8:1234::/48"]
                        .into_iter()
                        .map(|x| x.parse().unwrap())
                        .collect()
                )),
                std::sync::Arc::new(Object::Fqdn("example.com".to_string())),
            ])
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                (
                    "name".into(),
                    rhai::Dynamic::from("code_enhanced".to_string()),
                ),
                ("type".into(), rhai::Dynamic::from("code".to_string())),
                ("code".into(), rhai::Dynamic::from(550_i64)),
                ("enhanced".into(), rhai::Dynamic::from("5.7.2".to_string())),
                (
                    "text".into(),
                    rhai::Dynamic::from("nice to meet you, client".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Code(Reply::new(
                ReplyCode::Enhanced {
                    code: 550,
                    enhanced: "5.7.2".to_string(),
                },
                "nice to meet you, client".to_string()
            ))
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                ("name".into(), rhai::Dynamic::from("code".to_string())),
                ("type".into(), rhai::Dynamic::from("code".to_string())),
                ("code".into(), rhai::Dynamic::from(550_i64)),
                (
                    "text".into(),
                    rhai::Dynamic::from("nice to meet you, client".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Code(Reply::new(
                ReplyCode::Code { code: 550 },
                "nice to meet you, client".to_string()
            ))
        );

        assert_eq!(
            Object::from_map(&rhai::Map::from_iter([
                (
                    "name".into(),
                    rhai::Dynamic::from("code_from_string".to_string()),
                ),
                ("type".into(), rhai::Dynamic::from("code".to_string())),
                (
                    "value".into(),
                    rhai::Dynamic::from("220 nice to meet you, foobar".to_string()),
                ),
            ]))
            .unwrap(),
            Object::Code(Reply::new(
                ReplyCode::Code { code: 220 },
                "nice to meet you, foobar".to_string()
            ))
        );

        Object::from_map(&rhai::Map::from_iter([
            (
                "name".into(),
                rhai::Dynamic::from("inline code".to_string()),
            ),
            ("type".into(), rhai::Dynamic::from("code".to_string())),
            ("value".into(), rhai::Dynamic::from("250 ok".to_string())),
        ]))
        .unwrap();
    }
}
