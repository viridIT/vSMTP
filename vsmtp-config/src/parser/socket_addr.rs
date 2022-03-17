use vsmtp_common::libc_abstraction::if_nametoindex;

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<std::net::SocketAddr>, D::Error>
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

#[cfg(test)]
mod test {
    use vsmtp_common::libc_abstraction::{if_indextoname, if_nametoindex};

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct S {
        #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
        v: Vec<std::net::SocketAddr>,
    }

    #[test]
    fn socket_addr_ipv4() {
        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    25
                )]
            }
            .v,
            toml::from_str::<S>(r#"v = ["127.0.0.1:25"]"#).unwrap().v
        );

        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                    465
                )]
            }
            .v,
            toml::from_str::<S>(r#"v = ["0.0.0.0:465"]"#).unwrap().v
        );
    }

    #[test]
    fn socket_addr_ipv6() {
        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
                    25
                )]
            }
            .v,
            toml::from_str::<S>(r#"v = ["[::1]:25"]"#).unwrap().v
        );

        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
                    465
                )]
            }
            .v,
            toml::from_str::<S>(r#"v = ["[::]:465"]"#).unwrap().v
        );
    }

    #[test]
    fn socket_addr_ipv6_with_scope_id() {
        assert_eq!(
            format!(
                "{:?}",
                toml::from_str::<S>(r#"v = ["[::1%foobar]:25"]"#).unwrap_err()
            ),
            r#"Error { inner: ErrorInner { kind: Custom, line: Some(0), col: 0, at: Some(0), message: "if_nametoindex: 'No such device (os error 19)'", key: ["v"] } }"#
        );

        let interface1 = if_indextoname(1).unwrap();

        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
                    std::net::Ipv6Addr::LOCALHOST,
                    25,
                    0,
                    if_nametoindex(&interface1).unwrap(),
                ))]
            }
            .v,
            toml::from_str::<S>(&format!(r#"v = ["[::1%{interface1}]:25"]"#))
                .unwrap()
                .v
        );

        assert_eq!(
            S {
                v: vec![std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
                    std::net::Ipv6Addr::UNSPECIFIED,
                    465,
                    0,
                    if_nametoindex(&interface1).unwrap(),
                ))]
            }
            .v,
            toml::from_str::<S>(&format!(r#"v = ["[::%{interface1}]:465"]"#))
                .unwrap()
                .v
        );
    }
}
