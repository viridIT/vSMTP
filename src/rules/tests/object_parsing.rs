#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use crate::rules::obj::Object;
    use crate::rules::rule_engine;
    use crate::rules::tests::init;

    #[test]
    fn test_object_parsing_count() {
        println!("count");
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                assert_eq!(
                    rule_engine::acquire_engine().objects.read().unwrap().len(),
                    15
                );
            },
        );
    }

    #[test]
    fn test_object_parsing_ip4() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
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
            },
        );
    }

    #[test]
    fn test_object_parsing_fqdn() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
                let obj = objects.get("inline_fqdn");

                assert!(obj.is_some());
                match obj.unwrap() {
                    Object::Fqdn(value) => assert_eq!(*value, "xxx.com"),
                    _ => panic!("failed, objects tested aren't of type 'FQDN'."),
                }
            },
        );
    }

    #[test]
    fn test_object_parsing_val() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
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
            },
        );
    }

    #[test]
    fn test_object_parsing_addr() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
                let jones = objects.get("jones");
                let green = objects.get("green");

                assert!(jones.is_some());
                assert!(green.is_some());

                match (jones.unwrap(), green.unwrap()) {
                    (Object::Address(jones), Object::Address(green)) => {
                        assert_eq!(jones.full(), "jones@foo.com");
                        assert_eq!(green.full(), "green@bar.com");
                    }
                    _ => panic!("failed, objects tested aren't of type 'addr'."),
                }
            },
        );
    }

    #[test]
    fn test_object_parsing_file() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
                let whitelist = objects.get("whitelist");

                assert!(whitelist.is_some());

                match whitelist.unwrap() {
                    Object::File(content) => match &content[..] {
                        [Object::Address(green), Object::Address(jones), Object::Address(user)] => {
                            assert_eq!(green.full(), "green@bar.com");
                            assert_eq!(jones.full(), "jones@foo.com");
                            assert_eq!(user.full(), "user@domain.com");
                        }
                        _ => panic!("failed, objects tested aren't of type 'addr'."),
                    },
                    _ => panic!("failed, object tested isn't of type 'file'."),
                }
            },
        );
    }

    #[test]
    fn test_object_parsing_regex() {
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
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
            },
        );
    }

    #[test]
    fn test_object_parsing_groups() {
        println!("test_object_parsing_groups");
        init::test::run_engine_test(
            "./src/rules/tests/configs/objects-parsing.vsl",
            users::mock::MockUsers::with_current_uid(1),
            || {
                let engine = rule_engine::acquire_engine();

                let objects = engine.objects.read().unwrap();
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
                                        assert_eq!(green.full(), "green@bar.com");
                                        assert_eq!(jones.full(), "jones@foo.com");
                                        assert_eq!(user.full(), "user@domain.com");
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
                                                assert_eq!(green.full(), "green@bar.com");
                                                assert_eq!(jones.full(), "jones@foo.com");
                                                assert_eq!(user.full(), "user@domain.com");
                                            }
                                            _ => panic!(
                                                "failed, objects tested aren't of type 'addr'."
                                            ),
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
            },
        );
    }
}
