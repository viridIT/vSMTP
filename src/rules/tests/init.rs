#[cfg(test)]
pub(super) mod test {
    use std::panic;

    use crate::rules::rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE, RHAI_ENGINE};

    /// the rule engine uses a special architecture using a static variable
    /// to optimize performances. thus, it is difficult to test.
    /// this function wrapps a test routine to reset the rule engine
    /// for each test and execute tests in a defined order.
    ///
    /// run_engine_test takes an `order` number, the sources `src` used
    /// to reset the engine, `users` needed to run the test successfuly,
    /// using the *users* crate, and the `test` body.
    pub fn run_engine_test(
        src_path: &str,
        users: users::mock::MockUsers,
        test: impl Fn() + panic::RefUnwindSafe,
    ) {
        // re-initialize the engine.
        *RHAI_ENGINE.write().unwrap() = RhaiEngine::new(src_path, users)
            .unwrap_or_else(|_| panic!("couldn't initialize the engine for a test"));

        // getting a reader on the engine.
        let reader = RHAI_ENGINE
            .read()
            .expect("couldn't acquire the rhai engine for a test initialization");

        // evaluating scripts to parse objects and rules.
        reader
            .context
            .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &reader.ast)
            .expect("could not initialize the rule engine");

        // execute the test.
        test()
    }
}