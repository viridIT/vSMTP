#[cfg(test)]
mod test {
    use crate::rules::actions::*;
    use crate::rules::obj::*;
    use ipnet::*;
    use rhai::Map;
    use std::net::*;
    use std::str::FromStr;

    #[test]
    fn test_connect() {
        // ip4 / ip6
        {
            assert!(internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &Object::Ip4(Ipv4Addr::UNSPECIFIED),
            ),);
            assert!(!internal_is_connect(
                &IpAddr::V4(Ipv4Addr::LOCALHOST),
                &Object::Ip4(Ipv4Addr::UNSPECIFIED),
            ),);
            assert!(internal_is_connect(
                &IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                &Object::Ip6(Ipv6Addr::UNSPECIFIED),
            ),);
            assert!(!internal_is_connect(
                &IpAddr::V6(Ipv6Addr::LOCALHOST),
                &Object::Ip6(Ipv6Addr::UNSPECIFIED),
            ),);
        }

        // TODO: test ranges.
        {
            // assert!(internal_is_connect(
            //     &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            //     &Object::Rg4(["".parse::<Ipv4Net>().unwrap()].into_iter().collect()),
            // ),);
        }

        // regex.
        {
            assert!(internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &Object::Regex("^[a-z0-9.]+0.0$".parse().unwrap()),
            ),);
            assert!(internal_is_connect(
                &IpAddr::V4(Ipv4Addr::from_str("127.1.0.0").unwrap()),
                &Object::Regex("^[a-z0-9.]+0.0$".parse().unwrap()),
            ),);
            assert!(!internal_is_connect(
                &IpAddr::V4(Ipv4Addr::from_str("127.0.0.1").unwrap()),
                &Object::Regex("^[a-z0-9.]+0.0$".parse().unwrap()),
            ),);
        }

        // files & group.
        {
            let mut file = Map::new();
            file.insert("type".into(), "file".into());
            file.insert("content_type".into(), "addr".into());
            file.insert(
                "value".into(),
                "src/rules/tests/configs/whitelist.txt".into(),
            );

            let file = Object::from(&file).unwrap();

            assert!(!internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &file,
            ),);

            let group = Object::Group(vec![
                Object::Address("jones@foo.com".to_string()),
                Object::Ip4("0.0.0.0".parse().unwrap()),
            ]);

            assert!(internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &group,
            ),);
        }

        // invalid.
        {
            assert!(!internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &Object::Var("".to_string())
            ),);
            assert!(!internal_is_connect(
                &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                &Object::Fqdn("foo.com".to_string())
            ),);
        }
    }
}
