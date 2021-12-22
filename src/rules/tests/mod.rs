mod actions;
mod connect;
mod helo;
mod mail;
mod object_parsing;
mod rcpt;
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
    use std::{panic, sync::Once};

    static INIT: Once = Once::new();

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
        let config: ServerConfig = toml::from_str(
            &std::fs::read_to_string(config_path).expect("failed to read config from file"),
        )
        .unwrap();

        // init logs once.
        INIT.call_once(|| {
            log4rs::init_config(
                get_logger_config(&config).expect("couldn't initialize logs for a test"),
            )
            .expect("couldn't initialize logs for a test");
        });

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

        make_test::<T>(smtp_input, expected_output, config).await
    }

    fn get_logger_config(config: &ServerConfig) -> Result<log4rs::Config, std::io::Error> {
        use log4rs::*;

        let console = append::console::ConsoleAppender::builder()
            .encoder(Box::new(encode::pattern::PatternEncoder::new(
                "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
            )))
            .build();

        let file = append::file::FileAppender::builder()
            .encoder(Box::new(encode::pattern::PatternEncoder::new(
                "{d} - {m}{n}",
            )))
            .build(config.log.file.clone())?;

        Config::builder()
            .appender(config::Appender::builder().build("stdout", Box::new(console)))
            .appender(config::Appender::builder().build("file", Box::new(file)))
            .loggers(
                config
                    .log
                    .level
                    .iter()
                    .map(|(name, level)| config::Logger::builder().build(name, *level)),
            )
            .build(
                config::Root::builder()
                    .appender("stdout")
                    .appender("file")
                    .build(
                        *config
                            .log
                            .level
                            .get("default")
                            .unwrap_or(&log::LevelFilter::Warn),
                    ),
            )
            .map_err(|e| {
                e.errors().iter().for_each(|e| log::error!("{}", e));
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })
    }
}
