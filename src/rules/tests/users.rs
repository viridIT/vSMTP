#[cfg(test)]
mod test {
    use crate::rules::rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE};
    use lazy_static::lazy_static;
    use users::mock::MockUsers;

    // internals tests.

    lazy_static! {
        static ref TEST_ENGINE: RhaiEngine<MockUsers> = {
            let users = MockUsers::with_current_uid(1000);

            // TODO: add users here ...

            match RhaiEngine::new(include_bytes!("configs/users.vsl"), users) {
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
}
