#[allow(unused)]
macro_rules! generate_rule_check_test {
    ($init:expr, connect, $($against:expr, $should_be:ident),*) => {
        let obj = $init();
        $(
            println!("object {:?} {} connect {:?}", obj, if $should_be == true { "should be" } else {"should not be"}, $against);
            assert_eq!(
                internal_is_connect(&$against, &obj),
                $should_be
            );
        )*
    };
}

// TODO: generate those tests using a macro.
#[cfg(test)]
mod test {
    use crate::rules::actions::*;
    use crate::rules::obj::*;
    use rhai::Map;
    use std::net::*;

    #[test]
    fn test_connect() {
        // ip4 / ip6
        {
            generate_rule_check_test!(
                || Object::Ip4(Ipv4Addr::UNSPECIFIED),
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                true,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false
            );
            generate_rule_check_test!(
                || Object::Ip6(Ipv6Addr::UNSPECIFIED),
                connect,
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                true,
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                false
            );
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
            generate_rule_check_test!(
                || Object::Regex("^[a-z0-9.]+0.0$".parse().unwrap()),
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                true,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false,
                IpAddr::V4(Ipv4Addr::new(127, 90, 0, 0)),
                true
            );
        }

        // files & group.
        {
            generate_rule_check_test!(
                || {
                    let mut file = Map::new();
                    file.insert("type".into(), "file".into());
                    file.insert("content_type".into(), "addr".into());
                    file.insert(
                        "value".into(),
                        "src/rules/tests/configs/whitelist.txt".into(),
                    );

                    Object::from(&file).unwrap()
                },
                connect,
                // the whitelist doesn't contain ips, so everything is false.
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                false,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false,
                IpAddr::V4(Ipv4Addr::new(127, 90, 0, 0)),
                false
            );

            generate_rule_check_test!(
                || {
                    let mut file = Map::new();
                    file.insert("type".into(), "file".into());
                    file.insert("content_type".into(), "ip4".into());
                    file.insert("value".into(), "src/rules/tests/configs/hosts.txt".into());

                    Object::from(&file).unwrap()
                },
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                false,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                true,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
                true,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 93)),
                true,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 94)),
                false
            );

            generate_rule_check_test!(
                || {
                    Object::Group(vec![
                        Object::Address("jones@foo.com".to_string()),
                        Object::Ip4("0.0.0.0".parse().unwrap()),
                    ])
                },
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                true,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
                false
            );
        }

        // invalid.
        {
            generate_rule_check_test!(
                || Object::Var("".to_string()),
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                false,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
                false
            );

            generate_rule_check_test!(
                || Object::Fqdn("foo.com".to_string()),
                connect,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                false,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                false,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
                false
            );
        }
    }
}
