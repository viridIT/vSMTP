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
use std::{
    fs,
    io::{BufRead, BufReader},
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use super::address::Address;

/// Objects are rust's representation of rule engine variables.
/// multiple types are supported.
#[derive(Debug, Clone)]
pub enum Object {
    /// ip v4 address. (a.b.c.d)
    Ip4(Ipv4Addr),
    /// ip v6 address. (x:x:x:x:x:x:x:x)
    Ip6(Ipv6Addr),
    /// an ip v4 range. (a.b.c.d/range)
    Rg4(iprange::IpRange<ipnet::Ipv4Net>),
    /// an ip v6 range. (x:x:x:x:x:x:x:x/range)
    Rg6(iprange::IpRange<ipnet::Ipv6Net>),
    /// an email address (jones@foo.com)
    Address(Address),
    /// a valid fully qualified domain name (foo.com)
    Fqdn(String),
    /// a regex (^[a-z0-9.]+@foo.com$)
    Regex(regex::Regex),
    /// the content of a file.
    File(Vec<Object>),
    /// a group of objects declared inline.
    Group(Vec<std::sync::Arc<Object>>),
    /// a user.
    Identifier(String),
    /// a simple string.
    Str(String),
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ip4(l0), Self::Ip4(r0)) => l0 == r0,
            (Self::Ip6(l0), Self::Ip6(r0)) => l0 == r0,
            (Self::Rg4(l0), Self::Rg4(r0)) => l0 == r0,
            (Self::Rg6(l0), Self::Rg6(r0)) => l0 == r0,
            (Self::Address(l0), Self::Address(r0)) => l0 == r0,
            (Self::Fqdn(l0), Self::Fqdn(r0)) => l0 == r0,
            (Self::File(l0), Self::File(r0)) => l0 == r0,
            (Self::Group(l0), Self::Group(r0)) => l0 == r0,
            (Self::Identifier(l0), Self::Identifier(r0)) => l0 == r0,
            (Self::Str(l0), Self::Str(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Object {
    /// get a specific value from a rhai map and convert it to a specific type.
    /// returns an error if the cast failed.
    pub(crate) fn value<S, T>(
        map: &std::collections::BTreeMap<S, rhai::Dynamic>,
        key: &str,
    ) -> anyhow::Result<T>
    where
        S: FromStr + std::cmp::Ord,
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
    pub(crate) fn from<S>(
        map: &std::collections::BTreeMap<S, rhai::Dynamic>,
    ) -> anyhow::Result<Self>
    where
        S: std::fmt::Debug + FromStr + std::cmp::Ord + 'static,
    {
        let t = Object::value::<S, String>(map, "type")?;

        match t.as_str() {
            "ip4" => Ok(Object::Ip4(Ipv4Addr::from_str(
                &Object::value::<S, String>(map, "value")?,
            )?)),

            "ip6" => Ok(Object::Ip6(Ipv6Addr::from_str(
                &Object::value::<S, String>(map, "value")?,
            )?)),

            "rg4" => Ok(Object::Rg4(
                [Object::value::<S, String>(map, "value")?.parse::<ipnet::Ipv4Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "rg6" => Ok(Object::Rg6(
                [Object::value::<S, String>(map, "value")?.parse::<ipnet::Ipv6Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "fqdn" => {
                let value = Object::value::<S, String>(map, "value")?;
                match addr::parse_domain_name(&value) {
                    Ok(domain) => Ok(Object::Fqdn(domain.to_string())),
                    Err(_) => anyhow::bail!("'{}' is not a valid fqdn.", value),
                }
            }

            "addr" => {
                let value = Object::value::<S, String>(map, "value")?;
                Ok(Object::Address(Address::new(&value)?))
            }

            "ident" => Ok(Object::Identifier(Object::value::<S, String>(map, "value")?)),

            "str" => Ok(Object::Str(Object::value::<S, String>(map, "value")?)),

            "regex" => Ok(Object::Regex(regex::Regex::from_str(&Object::value::<
                S,
                String,
            >(
                map, "value"
            )?)?)),

            // the file object as an extra "content_type" parameter.
            "file" => {
                let value = Object::value::<S, String>(map, "value")?;
                let content_type = Object::value::<S, String>(map, "content_type")?;
                let reader = BufReader::new(fs::File::open(&value)?);
                let mut content = Vec::with_capacity(20);

                for line in reader.lines() {
                    match line {
                        Ok(line) => match content_type.as_str() {
                            "ip4" => content.push(Object::Ip4(Ipv4Addr::from_str(&line)?)),
                            "ip6" => content.push(Object::Ip6(Ipv6Addr::from_str(&line)?)),
                            "fqdn" => match addr::parse_domain_name(&line) {
                                Ok(domain) => content.push(Object::Fqdn(domain.to_string())),
                                Err(_) => anyhow::bail!("'{}' is not a valid fqdn.", value),
                            },
                            "addr" => content.push(Object::Address(Address::new(&line)?)),
                            "val" => content.push(Object::Str(line)),
                            "regex" => content.push(Object::Regex(regex::Regex::from_str(&line)?)),
                            _ => {}
                        },
                        Err(error) => log::error!("couldn't read line in '{}': {}", value, error),
                    };
                }

                Ok(Object::File(content))
            }

            "grp" => {
                let mut group = vec![];
                let elements = Object::value::<S, rhai::Array>(map, "value")?;
                let name = Object::value::<S, String>(map, "name")?;

                for element in elements.iter() {
                    group.push(
                        element
                            .clone()
                            .try_cast::<std::sync::Arc<Object>>()
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "an element inside the {name} group is not an object"
                                )
                            })?,
                    );
                }

                Ok(Object::Group(group))
            }

            _ => anyhow::bail!("'{}' is an unknown object type.", t),
        }
    }
}

impl ToString for Object {
    fn to_string(&self) -> String {
        match self {
            Object::Ip4(ip) => ip.to_string(),
            Object::Ip6(ip) => ip.to_string(),
            Object::Rg4(range) => format!("{:?}", range),
            Object::Rg6(range) => format!("{:?}", range),
            Object::Address(addr) => addr.to_string(),
            Object::Fqdn(fqdn) => fqdn.clone(),
            Object::Regex(regex) => regex.to_string(),
            Object::File(file) => format!("{file:?}"),
            Object::Group(grp) => format!("{grp:?}"),
            Object::Identifier(string) => string.clone(),
            Object::Str(string) => string.clone(),
        }
    }
}
