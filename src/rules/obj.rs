/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use ipnet::{Ipv4Net, Ipv6Net};
use iprange::IpRange;
use regex::Regex;
use rhai::{Array, Map};

use std::{
    fs,
    io::{BufRead, BufReader},
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use super::address::Address;

/// Objects are rust's representation of rule engine variables.
/// multiple types are supported.
#[derive(Debug)]
pub(super) enum Object {
    /// ip v4 address. (a.b.c.d)
    Ip4(Ipv4Addr),
    /// ip v6 address. (x:x:x:x:x:x:x:x)
    Ip6(Ipv6Addr),
    /// an ip v4 range. (a.b.c.d/range)
    Rg4(IpRange<Ipv4Net>),
    /// an ip v6 range. (x:x:x:x:x:x:x:x/range)
    Rg6(IpRange<Ipv6Net>),
    /// an email address (jones@foo.com)
    Address(Address),
    /// a valid fully qualified domain name (foo.com)
    Fqdn(String),
    /// a regex (^[a-z0-9.]+@foo.com$)
    Regex(Regex),
    /// the content of a file.
    File(Vec<Object>),
    /// a group of objects declared inline.
    Group(Vec<Object>),
    /// a generic variable.
    Var(String),
}

impl Object {
    // NOTE: what does the 'static lifetime implies here ?
    /// get a specific value from a rhai map and convert it to a specific type.
    /// returns an error if the cast failed.
    pub(crate) fn value<T: 'static + Clone>(map: &Map, key: &str) -> anyhow::Result<T> {
        match map.get(key) {
            Some(value) => value.clone().try_cast::<T>().ok_or_else(|| {
                anyhow::anyhow!("{} is not of type {}.", key, std::any::type_name::<T>())
            }),
            None => anyhow::bail!("{} not found.", key),
        }
    }

    /// create an object from a raw rhai Map data structure.
    /// this map must have the "value" and "type" keys to be parsed
    /// successfully.
    pub(crate) fn from(map: &Map) -> anyhow::Result<Self> {
        let t = Object::value::<String>(map, "type")?;

        match t.as_str() {
            "ip4" => Ok(Object::Ip4(Ipv4Addr::from_str(&Object::value::<String>(
                map, "value",
            )?)?)),

            "ip6" => Ok(Object::Ip6(Ipv6Addr::from_str(&Object::value::<String>(
                map, "value",
            )?)?)),

            "rg4" => Ok(Object::Rg4(
                [Object::value::<String>(map, "value")?.parse::<Ipv4Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "rg6" => Ok(Object::Rg6(
                [Object::value::<String>(map, "value")?.parse::<Ipv6Net>()?]
                    .into_iter()
                    .collect(),
            )),

            "fqdn" => {
                let value = Object::value::<String>(map, "value")?;
                match addr::parse_domain_name(&value) {
                    Ok(domain) => Ok(Object::Fqdn(domain.to_string())),
                    Err(_) => anyhow::bail!("'{}' is not a valid fqdn.", value),
                }
            }

            "addr" => {
                let value = Object::value::<String>(map, "value")?;
                Ok(Object::Address(Address::new(&value)?))
            }

            "val" => Ok(Object::Var(Object::value::<String>(map, "value")?)),

            "regex" => Ok(Object::Regex(Regex::from_str(&Object::value::<String>(
                map, "value",
            )?)?)),

            // the file object as an extra "content_type" parameter.
            "file" => {
                let value = Object::value::<String>(map, "value")?;
                let content_type = Object::value::<String>(map, "content_type")?;
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
                            "val" => content.push(Object::Var(line)),
                            "regex" => content.push(Object::Regex(Regex::from_str(&line)?)),
                            _ => {}
                        },
                        Err(error) => log::error!("couldn't read line in '{}': {}", value, error),
                    };
                }

                Ok(Object::File(content))
            }

            "grp" => {
                let mut group = vec![];
                let elements = Object::value::<Array>(map, "value")?;

                for element in elements.iter() {
                    match element.is::<Map>() {
                        true => group.push(Object::from(&element.clone_cast::<Map>())?),
                        false => {
                            let name = Object::value::<String>(map, "name")
                                .unwrap_or_else(|_| "unknown variable".to_string());
                            anyhow::bail!("'{name}' needs to be a map to be defined as a group.")
                        }
                    };
                }

                Ok(Object::Group(group))
            }

            _ => anyhow::bail!("'{}' is an unknown object type.", t),
        }
    }
}
