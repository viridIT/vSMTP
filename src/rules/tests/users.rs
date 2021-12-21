#[cfg(test)]
mod test {
    use crate::rules::rule_engine;
    use crate::rules::tests::init;

    #[tokio::test]
    async fn test_object_parsing_count() {
        let mut users = users::mock::MockUsers::with_current_uid(1);

        assert!(users.add_group(users::Group::new(100, "mail")).is_none());
        assert!(users.add_user(users::User::new(1, "jones", 100)).is_none());
        assert!(users.add_user(users::User::new(2, "green", 100)).is_none());
        assert!(users.add_user(users::User::new(3, "smith", 100)).is_none());

        async fn test() {
            let engine = rule_engine::acquire_engine();

            assert_eq!(engine.objects.read().unwrap().len(), 3);
        }

        init::run_engine_test("./src/rules/tests/rules/users/users.vsl", users, test).await;
    }

    #[tokio::test]
    async fn test_all_users_exists() {
        let mut users = users::mock::MockUsers::with_current_uid(1);

        assert!(users.add_group(users::Group::new(100, "mail")).is_none());
        assert!(users.add_user(users::User::new(1, "jones", 100)).is_none());
        assert!(users.add_user(users::User::new(2, "green", 100)).is_none());
        assert!(users.add_user(users::User::new(3, "smith", 100)).is_none());

        async fn test() {
            let mut scope = rule_engine::DEFAULT_SCOPE.clone();
            scope.push("__stage", "connect");

            let engine = rule_engine::acquire_engine();

            println!("objets: {:?}", engine.objects.read().unwrap());

            match engine
                .context
                .eval_ast_with_scope::<rule_engine::Status>(&mut scope, &engine.ast)
            {
                Ok(rule_engine::Status::Accept) => {}
                Ok(status) => panic!("the engine returned {:?} instead of Accept", status),
                Err(error) => panic!("engine returned an evaluation error: {}", error),
            }
        }

        init::run_engine_test("./src/rules/tests/rules/users/users.vsl", users, test).await;
    }
}
