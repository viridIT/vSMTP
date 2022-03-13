use vsmtp_common::libc_abstraction::{if_indextoname, if_nametoindex};

#[derive(Debug, PartialEq, serde::Deserialize)]
struct S {
    #[serde(deserialize_with = "crate::serializer::deserialize_socket_addr")]
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
        r#"Error { inner: ErrorInner { kind: Custom, line: Some(0), col: 0, at: Some(0), message: "Interface not found: 'foobar'", key: ["v"] } }"#
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
