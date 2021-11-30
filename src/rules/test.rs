#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use crate::rules::rule_engine::{RhaiEngine, Status, Var, DEFAULT_SCOPE};
    use lazy_static::lazy_static;

    // internals tests.

    lazy_static! {
        static ref TEST_ENGINE: RhaiEngine = {
            match RhaiEngine::from_bytes(include_bytes!("test-configs/objects-parsing.vsl")) {
                Ok(engine) => {
                    engine
                        .context
                        .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &engine.ast)
                        .expect("couldn't initialise the rule engine");

                    engine
                }
                Err(error) => {
                    eprintln!("object parsing failed: {}", error);
                    panic!("object parsing failed.");
                }
            }
        };
    }

    #[test]
    fn object_parsing_count() {
        assert_eq!(TEST_ENGINE.objects.read().unwrap().len(), 15);
    }

    #[test]
    fn object_parsing_ip4() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let unspecified = objects.get("unspecified");
        let localhost = objects.get("localhost");

        assert!(unspecified.is_some());
        assert!(localhost.is_some());

        match (unspecified.unwrap(), localhost.unwrap()) {
            (Var::Ip4(unspecified), Var::Ip4(localhost)) => {
                assert_eq!(*unspecified, Ipv4Addr::new(0, 0, 0, 0));
                assert_eq!(*localhost, Ipv4Addr::new(127, 0, 0, 1));
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_fqdn() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let var = objects.get("inline_fqdn");

        assert!(var.is_some());
        match var.unwrap() {
            Var::Fqdn(value) => assert_eq!(*value, "xxx.com"),
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_val() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let vals = vec![
            objects.get("user_dev"),
            objects.get("user_prod"),
            objects.get("user_test"),
        ];

        assert!(vals.iter().all(|val| val.is_some()));
        match vals.iter().map(|val| val.unwrap()).collect::<Vec<&Var>>()[..] {
            [Var::Val(user_dev), Var::Val(user_prod), Var::Val(user_test)] => {
                assert_eq!(*user_dev, "gitdev");
                assert_eq!(*user_prod, "gitproduction");
                assert_eq!(*user_test, "gittest");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_addr() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let jones = objects.get("jones");
        let green = objects.get("green");

        assert!(jones.is_some());
        assert!(green.is_some());

        match (jones.unwrap(), green.unwrap()) {
            (Var::Address(jones), Var::Address(green)) => {
                assert_eq!(*jones, "jones@foo.com");
                assert_eq!(*green, "green@bar.com");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_file() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let whitelist = objects.get("whitelist");

        assert!(whitelist.is_some());

        match whitelist.unwrap() {
            Var::File(content) => match &content[..] {
                [Var::Address(green), Var::Address(jones), Var::Address(user)] => {
                    assert_eq!(green.as_str(), "green@bar.com");
                    assert_eq!(jones.as_str(), "jones@foo.com");
                    assert_eq!(user.as_str(), "user@domain.com");
                }
                _ => assert!(false),
            },
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_regex() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let viridit_staff = objects.get("viridit_staff");
        let localhost_emails = objects.get("localhost_emails");

        assert!(viridit_staff.is_some());
        assert!(localhost_emails.is_some());

        match (viridit_staff.unwrap(), localhost_emails.unwrap()) {
            (Var::Regex(viridit_staff), Var::Regex(localhost_emails)) => {
                assert!(viridit_staff.is_match("some@viridit.com"));
                assert!(!viridit_staff.is_match("user@unknown.com"));
                assert!(localhost_emails.is_match("me@localhost"));
                assert!(!localhost_emails.is_match("user@notlocalhost.com"));
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn object_parsing_groups() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let authorized_users = objects.get("authorized_users");
        let deep_group = objects.get("deep_group");

        assert!(authorized_users.is_some());
        assert!(deep_group.is_some());

        match (authorized_users.unwrap(), deep_group.unwrap()) {
            (Var::Group(authorized_users), Var::Group(deep_group)) => {
                match &authorized_users[..] {
                    [Var::File(whitelist), Var::Ip4(authorized_ip)] => {
                        match &whitelist[..] {
                            [Var::Address(green), Var::Address(jones), Var::Address(user)] => {
                                assert_eq!(green.as_str(), "green@bar.com");
                                assert_eq!(jones.as_str(), "jones@foo.com");
                                assert_eq!(user.as_str(), "user@domain.com");
                            }
                            _ => assert!(false),
                        };

                        assert_eq!(*authorized_ip, Ipv4Addr::new(1, 1, 1, 1));
                    }
                    _ => assert!(false),
                };

                match &deep_group[..] {
                    [Var::Regex(foo_emails), Var::Group(authorized_users)] => {
                        assert!(foo_emails.is_match("jones@foo.com"));
                        assert!(!foo_emails.is_match("green@bar.com"));

                        // nested group, same objcet as tested above.
                        match &authorized_users[..] {
                            [Var::File(whitelist), Var::Ip4(authorized_ip)] => {
                                match &whitelist[..] {
                                    [Var::Address(green), Var::Address(jones), Var::Address(user)] =>
                                    {
                                        assert_eq!(green.as_str(), "green@bar.com");
                                        assert_eq!(jones.as_str(), "jones@foo.com");
                                        assert_eq!(user.as_str(), "user@domain.com");
                                    }
                                    _ => assert!(false),
                                };

                                assert_eq!(*authorized_ip, Ipv4Addr::new(1, 1, 1, 1));
                            }
                            _ => assert!(false),
                        };
                    }
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }
    }
}
