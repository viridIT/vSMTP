mod actions;
mod connect;
mod helo;
mod object_parsing;
mod users;

#[cfg(test)]
pub mod helpers {
    use crate::{
        config::server_config::ServerConfig,
        mailprocessing::mail_receiver::MailReceiver,
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE, RHAI_ENGINE},
        smtp::code::SMTPReplyCode,
        tests::Mock,
    };
    use std::panic;

    struct DefaultResolverTest;

    #[async_trait::async_trait]
    impl DataEndResolver for DefaultResolverTest {
        async fn on_data_end(
            _: &ServerConfig,
            _: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            Ok(SMTPReplyCode::Code250)
        }
    }

    pub async fn make_test<T: crate::resolver::DataEndResolver>(
        smtp_input: &[u8],
        expected_output: &[u8],
        mut config: ServerConfig,
    ) -> Result<(), std::io::Error> {
        config.prepare();

        let mut receiver = MailReceiver::<T>::new(
            "0.0.0.0:0".parse().unwrap(),
            None,
            std::sync::Arc::new(config),
        );
        let mut write = Vec::new();
        let mock = Mock::new(smtp_input.to_vec(), &mut write);

        match receiver.receive_plain(mock).await {
            Ok(mut mock) => {
                let _ = std::io::Write::flush(&mut mock);
                assert_eq!(
                    std::str::from_utf8(&write),
                    std::str::from_utf8(&expected_output.to_vec())
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// the rule engine uses a special architecture using a static variable
    /// to optimize performances. thus, it is difficult to test.
    /// this function wrapps a test routine to reset the rule engine
    /// for each test and execute tests in a defined order.
    ///
    /// run_engine_test takes the sources path `src_path` of the script used
    /// to reset the engine, `users` needed to run the test successfuly,
    /// using the *users* crate, and the `test` body.
    pub fn run_engine_test<F>(src_path: &str, users: users::mock::MockUsers, test: F)
    where
        F: Fn() + panic::RefUnwindSafe,
    {
        // re-initialize the engine.
        *RHAI_ENGINE.write().unwrap() = RhaiEngine::new(src_path, users)
            .unwrap_or_else(|error| panic!("couldn't initialize the engine for a test: {}", error));

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

    /// the rule engine uses a special architecture using a static variable
    /// to optimize performances. thus, it is difficult to test.
    /// this function wrapps emulates the behavior of vsmtp's state machine
    /// while using a fresh rule engine for every tests.
    ///
    /// it takes the sources (`src_path`) and configuration (`config_path`) paths of the script used
    /// to reset the engine, `users` needed to run the test successfuly,
    /// (using the *users* crate) the commands to send to the state machine
    /// and the expected output of the server.
    pub async fn run_integration_engine_test<T: DataEndResolver>(
        src_path: &str,
        config_path: &str,
        users: users::mock::MockUsers,
        smtp_input: &[u8],
        expected_output: &[u8],
    ) -> Result<(), std::io::Error> {
        // re-initialize the engine.
        *RHAI_ENGINE.write().unwrap() = RhaiEngine::new(src_path, users)
            .unwrap_or_else(|error| panic!("couldn't initialize the engine for a test: {}", error));

        // getting a reader on the engine.
        let reader = RHAI_ENGINE
            .read()
            .expect("couldn't acquire the rhai engine for a test initialization");

        // evaluating scripts to parse objects and rules.
        reader
            .context
            .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &reader.ast)
            .expect("could not initialize the rule engine");

        make_test::<T>(
            smtp_input,
            expected_output,
            toml::from_str(
                &std::fs::read_to_string(config_path).expect("failed to read config from file"),
            )
            .expect("cannot parse config from toml"),
        )
        .await
    }
}
