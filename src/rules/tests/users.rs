#[cfg(test)]
mod test {
    use crate::rules::rule_engine;

    #[ignore]
    #[test]
    fn test_object_parsing_count() {
        rule_engine::init("./src/rules/tests/configs/users.vsl");

        assert_eq!(rule_engine::RHAI_ENGINE.objects.read().unwrap().len(), 3);
    }

    #[ignore]
    #[test]
    fn test_all_users_exists() {
        rule_engine::init("./src/rules/tests/configs/users.vsl");

        let mut scope = rule_engine::DEFAULT_SCOPE.clone();

        scope.push("__stage", "connect");

        match rule_engine::RHAI_ENGINE
            .context
            .eval_ast_with_scope::<rule_engine::Status>(&mut scope, &rule_engine::RHAI_ENGINE.ast)
        {
            Ok(rule_engine::Status::Accept) => {}
            Ok(status) => panic!("the engine returned {:?} instead of Accept", status),
            Err(error) => panic!("engine returned an evaluation error: {}", error),
        }
    }
}
