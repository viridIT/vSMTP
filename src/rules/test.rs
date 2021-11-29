#[cfg(test)]
mod test {
    use crate::rules::rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE};

    // internals tests.

    #[test]
    fn object_parsing() {
        let objects_scirpt = include_bytes!("test-configs/objects-parsing.vsl");

        let engine = match RhaiEngine::from_bytes(objects_scirpt) {
            Ok(engine) => engine,
            Err(error) => {
                eprintln!("object parsing failed: {}", error);
                panic!("object parsing failed.");
            }
        };

        engine
            .context
            .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &engine.ast)
            .expect("couldn't initialise the rule engine");

        assert_eq!(engine.objects.read().unwrap().len(), 15);
    }
}
