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
use vsmtp_common::{re::anyhow, utils::ipv6_with_scope_id};

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<std::net::SocketAddr>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Vec<String> as serde::Deserialize>::deserialize(deserializer)?
        .into_iter()
        .map(|s| {
            <std::net::SocketAddr as std::str::FromStr>::from_str(&s)
                .or_else(|_| ipv6_with_scope_id(&s))
        })
        .collect::<anyhow::Result<Vec<std::net::SocketAddr>>>()
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod test {
    use vsmtp_common::libc_abstraction::{if_indextoname, if_nametoindex};
    use vsmtp_common::re::serde_json;

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct S {
        #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
        v: Vec<std::net::SocketAddr>,
    }

    #[test]
    fn error() {
        assert!(serde_json::from_str::<S>(r#"{"v": ["foobar"]}"#).is_err());
    }

    #[test]
    fn socket_addr_ipv4() {
        assert_eq!(
            vec![std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                25
            )],
            serde_json::from_str::<S>(r#"{"v": ["127.0.0.1:25"]}"#)
                .unwrap()
                .v
        );

        assert_eq!(
            vec![std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                465
            )],
            serde_json::from_str::<S>(r#"{"v": ["0.0.0.0:465"]}"#)
                .unwrap()
                .v
        );
    }

    #[test]
    fn socket_addr_ipv6() {
        assert_eq!(
            vec![std::net::SocketAddr::new(
                std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
                25
            )],
            serde_json::from_str::<S>(r#"{"v": ["[::1]:25"]}"#)
                .unwrap()
                .v
        );

        assert_eq!(
            vec![std::net::SocketAddr::new(
                std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
                465
            )],
            serde_json::from_str::<S>(r#"{"v": ["[::]:465"]}"#)
                .unwrap()
                .v
        );
    }

    #[test]
    fn socket_addr_ipv6_with_scope_id() {
        let interface1 = if_indextoname(1).unwrap();

        assert_eq!(
            vec![std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
                std::net::Ipv6Addr::LOCALHOST,
                25,
                0,
                if_nametoindex(&interface1).unwrap(),
            ))],
            serde_json::from_str::<S>(&format!(r#"{{"v": ["[::1%{interface1}]:25"]}}"#))
                .unwrap()
                .v
        );

        assert_eq!(
            vec![std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
                std::net::Ipv6Addr::UNSPECIFIED,
                465,
                0,
                if_nametoindex(&interface1).unwrap(),
            ))],
            serde_json::from_str::<S>(&format!(r#"{{"v": ["[::%{interface1}]:465"]}}"#))
                .unwrap()
                .v
        );
    }

    #[test]
    fn socket_addr_ipv6_with_scope_id_error() {
        assert!(serde_json::from_str::<S>(r#"{"v": ["[::1%foobar]"]}"#)
            .unwrap_err()
            .is_data());

        assert!(serde_json::from_str::<S>(r#"{"v": ["::1%foobar:25"]}"#)
            .unwrap_err()
            .is_data());

        assert!(serde_json::from_str::<S>(r#"{"v": ["[::1!scope_id]:25"]}"#)
            .unwrap_err()
            .is_data());
    }
}
