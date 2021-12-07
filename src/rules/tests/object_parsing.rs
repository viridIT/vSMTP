#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use crate::rules::{
        obj::Object,
        rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE},
    };
    use lazy_static::lazy_static;

    // internals tests.

    lazy_static! {
        static ref TEST_ENGINE: RhaiEngine = {
            match RhaiEngine::from_bytes(include_bytes!("configs/objects-parsing.vsl")) {
                Ok(engine) => {
                    engine
                        .context
                        .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &engine.ast)
                        .expect("couldn't initialize the rule engine");

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
            (Object::Ip4(unspecified), Object::Ip4(localhost)) => {
                assert_eq!(*unspecified, Ipv4Addr::new(0, 0, 0, 0));
                assert_eq!(*localhost, Ipv4Addr::new(127, 0, 0, 1));
            }
            _ => panic!("failed, objects tested aren't of type 'Ipv(4/6)'."),
        }
    }

    #[test]
    fn object_parsing_fqdn() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let obj = objects.get("inline_fqdn");

        assert!(obj.is_some());
        match obj.unwrap() {
            Object::Fqdn(value) => assert_eq!(*value, "xxx.com"),
            _ => panic!("failed, objects tested aren't of type 'FQDN'."),
        }
    }

    #[test]
    fn object_parsing_val() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let vars = vec![
            objects.get("user_dev"),
            objects.get("user_prod"),
            objects.get("user_test"),
        ];

        assert!(vars.iter().all(|val| val.is_some()));
        match vars
            .iter()
            .map(|val| val.unwrap())
            .collect::<Vec<&Object>>()[..]
        {
            [Object::Var(user_dev), Object::Var(user_prod), Object::Var(user_test)] => {
                assert_eq!(*user_dev, "gitdev");
                assert_eq!(*user_prod, "gitproduction");
                assert_eq!(*user_test, "gittest");
            }
            _ => panic!("failed, objects tested aren't of type 'Var'."),
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
            (Object::Address(jones), Object::Address(green)) => {
                assert_eq!(*jones, "jones@foo.com");
                assert_eq!(*green, "green@bar.com");
            }
            _ => panic!("failed, objects tested aren't of type 'addr'."),
        }
    }

    #[test]
    fn object_parsing_file() {
        let objects = TEST_ENGINE.objects.read().unwrap();
        let whitelist = objects.get("whitelist");

        assert!(whitelist.is_some());

        match whitelist.unwrap() {
            Object::File(content) => match &content[..] {
                [Object::Address(green), Object::Address(jones), Object::Address(user)] => {
                    assert_eq!(green.as_str(), "green@bar.com");
                    assert_eq!(jones.as_str(), "jones@foo.com");
                    assert_eq!(user.as_str(), "user@domain.com");
                }
                _ => panic!("failed, objects tested aren't of type 'addr'."),
            },
            _ => panic!("failed, object tested isn't of type 'file'."),
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
            (Object::Regex(viridit_staff), Object::Regex(localhost_emails)) => {
                assert!(viridit_staff.is_match("some@viridit.com"));
                assert!(!viridit_staff.is_match("user@unknown.com"));
                assert!(localhost_emails.is_match("me@localhost"));
                assert!(!localhost_emails.is_match("user@notlocalhost.com"));
            }
            _ => panic!("failed, objects tested aren't of type 'regex'."),
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
            (Object::Group(authorized_users), Object::Group(deep_group)) => {
                match &authorized_users[..] {
                    [Object::File(whitelist), Object::Ip4(authorized_ip)] => {
                        match &whitelist[..] {
                            [Object::Address(green), Object::Address(jones), Object::Address(user)] =>
                            {
                                assert_eq!(green.as_str(), "green@bar.com");
                                assert_eq!(jones.as_str(), "jones@foo.com");
                                assert_eq!(user.as_str(), "user@domain.com");
                            }
                            _ => panic!("failed, objects tested aren't of type 'addr'."),
                        };

                        assert_eq!(*authorized_ip, Ipv4Addr::new(1, 1, 1, 1));
                    }
                    _ => panic!("failed, objects tested aren't of type 'grp'."),
                };

                match &deep_group[..] {
                    [Object::Regex(foo_emails), Object::Group(authorized_users)] => {
                        assert!(foo_emails.is_match("jones@foo.com"));
                        assert!(!foo_emails.is_match("green@bar.com"));

                        // nested group, same object as tested above.
                        match &authorized_users[..] {
                            [Object::File(whitelist), Object::Ip4(authorized_ip)] => {
                                match &whitelist[..] {
                                    [Object::Address(green), Object::Address(jones), Object::Address(user)] =>
                                    {
                                        assert_eq!(green.as_str(), "green@bar.com");
                                        assert_eq!(jones.as_str(), "jones@foo.com");
                                        assert_eq!(user.as_str(), "user@domain.com");
                                    }
                                    _ => panic!("failed, objects tested aren't of type 'addr'."),
                                };

                                assert_eq!(*authorized_ip, Ipv4Addr::new(1, 1, 1, 1));
                            }
                            _ => panic!("failed, objects tested aren't of type 'grp'."),
                        };
                    }
                    _ => panic!("failed, objects tested aren't of type 'grp'."),
                }
            }
            _ => panic!("failed, objects tested aren't of type 'grp'."),
        }
    }
}
