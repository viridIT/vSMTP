// // TODO: create a macro that generates sets of data examples.

// #[allow(unused)]
// macro_rules! generate_rule_check_test {
//     ($init:expr, connect, $($against:expr, $should_be:ident),*) => {
//         let obj = $init();
//         $(
//             println!("object {:?} {} connect {:?}", obj, if $should_be == true { "should be" } else {"should not be"}, $against);
//             assert_eq!(
//                 internal_is_connect(&$against, &obj),
//                 $should_be
//             );
//         )*
//     };
//     ($init:expr, helo, $($against:expr, $should_be:ident),*) => {
//         let obj = $init();
//         $(
//             println!("object {:?} {} helo {:?}", obj, if $should_be == true { "should be" } else {"should not be"}, $against);
//             assert_eq!(
//                 internal_is_helo(&$against, &obj),
//                 $should_be
//             );
//         )*
//     };
//     ($init:expr, mail, $($against:expr, $should_be:ident),*) => {
//         let obj = $init();
//         $(
//             println!("object {:?} {} mail {:?}", obj, if $should_be == true { "should be" } else {"should not be"}, $against);
//             assert_eq!(
//                 internal_is_mail(&$against, &obj),
//                 $should_be
//             );
//         )*
//     };
//     ($init:expr, rcpt, $($against:expr, $should_be:ident),*) => {
//         let obj = $init();
//         $(
//             println!("object {:?} {} rcpt {:?}", obj, if $should_be == true { "should be" } else {"should not be"}, $against);
//             assert_eq!(
//                 internal_is_rcpt(&$against, &obj),
//                 $should_be
//             );
//         )*
//     };
// }

// // TODO: generate those tests using a macro.
// #[cfg(test)]
// mod test {
//     use crate::rules::actions::*;
//     use crate::rules::address::Address;
//     use crate::rules::obj::*;
//     use rhai::Map;
//     use std::net::*;

//     #[test]
//     fn test_connect() {
//         // ip4 / ip6
//         {
//             generate_rule_check_test!(
//                 || Object::Ip4(Ipv4Addr::UNSPECIFIED),
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 true,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false
//             );
//             generate_rule_check_test!(
//                 || Object::Ip6(Ipv6Addr::UNSPECIFIED),
//                 connect,
//                 IpAddr::V6(Ipv6Addr::UNSPECIFIED),
//                 true,
//                 IpAddr::V6(Ipv6Addr::LOCALHOST),
//                 false
//             );
//         }

//         // TODO: test ranges.
//         {
//             // assert!(internal_is_connect(
//             //     &IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//             //     &Object::Rg4(["".parse::<Ipv4Net>().unwrap()].into_iter().collect()),
//             // ),);
//         }

//         // regex.
//         {
//             generate_rule_check_test!(
//                 || Object::Regex("^[a-z0-9.]+0.0$".parse().unwrap()),
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 true,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false,
//                 IpAddr::V4(Ipv4Addr::new(127, 90, 0, 0)),
//                 true
//             );
//         }

//         // files & group.
//         {
//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "addr".into());
//                     file.insert(
//                         "value".into(),
//                         "src/rules/tests/configs/whitelist.txt".into(),
//                     );

//                     Object::from(&file).unwrap()
//                 },
//                 connect,
//                 // the whitelist doesn't contain ips, so everything is false.
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 false,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false,
//                 IpAddr::V4(Ipv4Addr::new(127, 90, 0, 0)),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "ip4".into());
//                     file.insert("value".into(), "src/rules/tests/configs/hosts.txt".into());

//                     Object::from(&file).unwrap()
//                 },
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 false,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 true,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
//                 true,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 93)),
//                 true,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 94)),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     Object::Group(vec![
//                         Object::Address(Address::new("jones@foo.com").unwrap()),
//                         Object::Ip4("0.0.0.0".parse().unwrap()),
//                     ])
//                 },
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 true,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
//                 false
//             );
//         }

//         // invalid.
//         {
//             generate_rule_check_test!(
//                 || Object::Var("".to_string()),
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 false,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
//                 false
//             );

//             generate_rule_check_test!(
//                 || Object::Fqdn("foo.com".to_string()),
//                 connect,
//                 IpAddr::V4(Ipv4Addr::UNSPECIFIED),
//                 false,
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 false,
//                 IpAddr::V4(Ipv4Addr::new(192, 168, 1, 91)),
//                 false
//             );
//         }
//     }

//     #[test]
//     fn test_helo() {
//         // fqdn.
//         {
//             generate_rule_check_test!(
//                 || Object::Fqdn("foo.com".to_string()),
//                 helo,
//                 "foo.com",
//                 true,
//                 "bar.com",
//                 false
//             );
//         }

//         // regex.
//         {
//             generate_rule_check_test!(
//                 || Object::Regex("^[a-z0-9.]+.com$".parse().unwrap()),
//                 helo,
//                 "foo.com",
//                 true,
//                 "bar.com",
//                 true,
//                 "foo.bar",
//                 false
//             );
//         }

//         // files & group.
//         {
//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "fqdn".into());
//                     file.insert("value".into(), "src/rules/tests/configs/domains.txt".into());

//                     Object::from(&file).unwrap()
//                 },
//                 helo,
//                 "foo.bar",
//                 true,
//                 "viridit.com",
//                 true,
//                 "satan.fr",
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "ip4".into());
//                     file.insert("value".into(), "src/rules/tests/configs/hosts.txt".into());

//                     Object::from(&file).unwrap()
//                 },
//                 helo,
//                 // nothing matches because content isn't of fqdn type.
//                 "foo.bar",
//                 false,
//                 "viridit.com",
//                 false,
//                 "satan.fr",
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     Object::Group(vec![
//                         Object::Address(Address::new("jones@foo.com").unwrap()),
//                         Object::Fqdn("foo.com".to_string()),
//                         Object::Ip4("0.0.0.0".parse().unwrap()),
//                     ])
//                 },
//                 helo,
//                 "foo.bar",
//                 false,
//                 "viridit.com",
//                 false,
//                 "foo.com",
//                 true
//             );
//         }

//         // invalid.
//         {
//             generate_rule_check_test!(
//                 || Object::Var("".to_string()),
//                 helo,
//                 "foo.bar",
//                 false,
//                 "viridit.com",
//                 false,
//                 "foo.com",
//                 false
//             );

//             generate_rule_check_test!(
//                 || Object::Ip4(Ipv4Addr::UNSPECIFIED),
//                 helo,
//                 "foo.bar",
//                 false,
//                 "viridit.com",
//                 false,
//                 "foo.com",
//                 false
//             );
//             generate_rule_check_test!(
//                 || Object::Ip6(Ipv6Addr::UNSPECIFIED),
//                 helo,
//                 "foo.bar",
//                 false,
//                 "viridit.com",
//                 false,
//                 "foo.com",
//                 false
//             );
//         }
//     }

//     #[test]
//     fn test_mail() {
//         // addr.
//         {
//             generate_rule_check_test!(
//                 || Object::Address(Address::new("jones@foo.com").unwrap()),
//                 mail,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@bar.com").unwrap(),
//                 false,
//                 Address::new("green@foo.com").unwrap(),
//                 false
//             );
//         }

//         // var / user
//         {
//             generate_rule_check_test!(
//                 || Object::Var("green".to_string()),
//                 mail,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("green@bar.com").unwrap(),
//                 true
//             );
//         }

//         // fqdn
//         {
//             generate_rule_check_test!(
//                 || Object::Fqdn("foo.com".to_string()),
//                 mail,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("green@bar.com").unwrap(),
//                 false
//             );
//         }

//         // regex.
//         {
//             generate_rule_check_test!(
//                 || Object::Regex("^[a-z0-9.]+@foo.com$".parse().unwrap()),
//                 mail,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@bar.com").unwrap(),
//                 false,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("viridit.staff@foo.com").unwrap(),
//                 true
//             );
//         }

//         // files & group.
//         {
//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "addr".into());
//                     file.insert(
//                         "value".into(),
//                         "src/rules/tests/configs/whitelist.txt".into(),
//                     );

//                     Object::from(&file).unwrap()
//                 },
//                 mail,
//                 Address::new("green@bar.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("unknown@user.com").unwrap(),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "ip4".into());
//                     file.insert("value".into(), "src/rules/tests/configs/hosts.txt".into());

//                     Object::from(&file).unwrap()
//                 },
//                 mail,
//                 // nothing matches because content isn't of addr type.
//                 Address::new("green@bar.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("unknown@user.com").unwrap(),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     Object::Group(vec![
//                         Object::Address(Address::new("jones@foo.com").unwrap()),
//                         Object::Fqdn("x.com".to_string()),
//                         Object::Ip4("0.0.0.0".parse().unwrap()),
//                     ])
//                 },
//                 mail,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//         }

//         // invalid
//         {
//             generate_rule_check_test!(
//                 || Object::Ip4(Ipv4Addr::UNSPECIFIED),
//                 mail,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//             generate_rule_check_test!(
//                 || Object::Ip6(Ipv6Addr::UNSPECIFIED),
//                 mail,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//         }
//     }

//     #[test]
//     fn test_rcpt() {
//         // addr.
//         {
//             generate_rule_check_test!(
//                 || Object::Address(Address::new("jones@foo.com").unwrap()),
//                 rcpt,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@bar.com").unwrap(),
//                 false,
//                 Address::new("green@foo.com").unwrap(),
//                 false
//             );
//         }

//         // var / user
//         {
//             generate_rule_check_test!(
//                 || Object::Var("green".to_string()),
//                 mail,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("green@bar.com").unwrap(),
//                 true
//             );
//         }

//         // fqdn
//         {
//             generate_rule_check_test!(
//                 || Object::Fqdn("foo.com".to_string()),
//                 mail,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("green@bar.com").unwrap(),
//                 false
//             );
//         }

//         // regex.
//         {
//             generate_rule_check_test!(
//                 || Object::Regex("^[a-z0-9.]+@foo.com$".parse().unwrap()),
//                 rcpt,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("jones@bar.com").unwrap(),
//                 false,
//                 Address::new("green@foo.com").unwrap(),
//                 true,
//                 Address::new("viridit.staff@foo.com").unwrap(),
//                 true
//             );
//         }

//         // files & group.
//         {
//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "addr".into());
//                     file.insert(
//                         "value".into(),
//                         "src/rules/tests/configs/whitelist.txt".into(),
//                     );

//                     Object::from(&file).unwrap()
//                 },
//                 rcpt,
//                 Address::new("green@bar.com").unwrap(),
//                 true,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("unknown@user.com").unwrap(),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     let mut file = Map::new();
//                     file.insert("type".into(), "file".into());
//                     file.insert("content_type".into(), "ip4".into());
//                     file.insert("value".into(), "src/rules/tests/configs/hosts.txt".into());

//                     Object::from(&file).unwrap()
//                 },
//                 rcpt,
//                 // nothing matches because content isn't of addr type.
//                 Address::new("green@bar.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("unknown@user.com").unwrap(),
//                 false
//             );

//             generate_rule_check_test!(
//                 || {
//                     Object::Group(vec![
//                         Object::Address(Address::new("jones@foo.com").unwrap()),
//                         Object::Fqdn("x.com".to_string()),
//                         Object::Ip4("0.0.0.0".parse().unwrap()),
//                     ])
//                 },
//                 rcpt,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 true,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//         }

//         // invalid.
//         {
//             generate_rule_check_test!(
//                 || Object::Ip4(Ipv4Addr::UNSPECIFIED),
//                 rcpt,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//             generate_rule_check_test!(
//                 || Object::Ip6(Ipv6Addr::UNSPECIFIED),
//                 rcpt,
//                 Address::new("test@foo.com").unwrap(),
//                 false,
//                 Address::new("jones@foo.com").unwrap(),
//                 false,
//                 Address::new("other@user.com").unwrap(),
//                 false
//             );
//         }
//     }
// }
