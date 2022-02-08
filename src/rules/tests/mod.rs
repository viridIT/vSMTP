/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
mod actions;
mod connect;
mod helo;
mod mail;
mod object_parsing;
mod r_lookup;
mod rcpt;
mod users;

#[cfg(test)]
pub mod helpers {
    use crate::{
        collection,
        config::{get_logger_config, server_config::ServerConfig},
        receiver::test_helpers::test_receiver,
        resolver::Resolver,
        // rules::rule_engine::{RhaiEngine, Status, DEFAULT_SCOPE, RHAI_ENGINE},
    };
    use std::{panic, sync::Once};

    static INIT: Once = Once::new();

    /// the rule engine uses a special architecture using a static variable
    /// to optimize performances. thus, it is difficult to test.
    /// this function wraps a test routine to reset the rule engine
    /// for each test and execute tests in a defined order.
    ///
    /// run_engine_test takes the sources path `src_path` of the script used
    /// to reset the engine, `users` needed to run the test successfully,
    /// using the *users* crate, and the `test` body.
    pub fn run_engine_test<F>(src_path: &str, users: users::mock::MockUsers, test: F)
    where
        F: Fn() + panic::RefUnwindSafe,
    {
        // re-initialize the engine.
        // *RHAI_ENGINE.write().unwrap() = RhaiEngine::new(src_path, users)
        //     .unwrap_or_else(|error| panic!("couldn't initialize the engine for a test: {}", error));

        // // getting a reader on the engine.
        // let reader = RHAI_ENGINE
        //     .read()
        //     .expect("couldn't acquire the rhai engine for a test initialization");

        // // evaluating scripts to parse objects and rules.
        // reader
        //     .context
        //     .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &reader.ast)
        //     .expect("could not initialize the rule engine");

        // execute the test.
        test()
    }

    /// the rule engine uses a special architecture using a static variable
    /// to optimize performances. thus, it is difficult to test.
    /// this function wraps emulates the behavior of vsmtp's state machine
    /// while using a fresh rule engine for every tests.
    ///
    /// it takes the sources (`src_path`) and configuration (`config_path`) paths of the script used
    /// to reset the engine, `users` needed to run the test successfully,
    /// (using the *users* crate) the commands to send to the state machine
    /// and the expected output of the server.
    pub async fn run_integration_engine_test<T>(
        address: &str,
        resolver: T,
        src_path: &str,
        users: users::mock::MockUsers,
        smtp_input: &[u8],
        expected_output: &[u8],
    ) -> anyhow::Result<()>
    where
        T: Resolver + Send + Sync + 'static,
    {
        let config = ServerConfig::builder()
            .with_server_default_port("test.server.com")
            .with_logging(
                "./tests/generated/output.log",
                collection! {"default".to_string() => log::LevelFilter::Error},
            )
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tests/generated/spool/", collection! {})
            .with_rules(src_path)
            .with_default_reply_codes()
            .build();

        // init logs once.
        INIT.call_once(|| {
            log4rs::init_config(
                get_logger_config(&config).expect("couldn't initialize logs for a test"),
            )
            .expect("couldn't initialize logs for a test");
        });

        // re-initialize the engine.
        // *RHAI_ENGINE.write().unwrap() = RhaiEngine::new(src_path, users)
        //     .unwrap_or_else(|error| panic!("couldn't initialize the engine for a test: {}", error));

        // // getting a reader on the engine.
        // let reader = RHAI_ENGINE
        //     .read()
        //     .expect("couldn't acquire the rhai engine for a test initialization");

        // // evaluating scripts to parse objects and rules.
        // reader
        //     .context
        //     .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &reader.ast)
        //     .expect("failed to run the rule engine");

        test_receiver(
            address,
            resolver,
            smtp_input,
            expected_output,
            std::sync::Arc::new(config),
        )
        .await
    }
}
