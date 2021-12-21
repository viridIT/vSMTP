#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::{
            tests::init::run_engine_test,
            tests::test::{get_test_config, make_test},
        },
        smtp::code::SMTPReplyCode,
    };

    struct Test;

    #[async_trait::async_trait]
    impl DataEndResolver for Test {
        async fn on_data_end(
            _: &ServerConfig,
            _: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_valid_helo() {
        async fn test() {
            assert!(make_test::<Test>(
                ["HELO viridit.com\r\n"].concat().as_bytes(),
                ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        async fn test2() {
            assert!(make_test::<Test>(
                ["HELO ibm.com\r\n"].concat().as_bytes(),
                [
                    "220 test.server.com Service ready\r\n",
                    "554 permanent problems with the remote server\r\n"
                ]
                .concat()
                .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        run_engine_test(
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            test,
        )
        .await;

        run_engine_test(
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            test2,
        )
        .await;
    }

    #[tokio::test]
    async fn test_types_helo() {
        async fn regex() {
            assert!(make_test::<Test>(
                ["HELO viridit.eu\r\n"].concat().as_bytes(),
                ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        async fn regex2() {
            assert!(make_test::<Test>(
                ["HELO viridit.com\r\n"].concat().as_bytes(),
                [
                    "220 test.server.com Service ready\r\n",
                    "554 permanent problems with the remote server\r\n"
                ]
                .concat()
                .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        async fn file() {
            assert!(make_test::<Test>(
                ["HELO viridit.fr\r\n"].concat().as_bytes(),
                ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        async fn file2() {
            assert!(make_test::<Test>(
                ["HELO green.foo\r\n"].concat().as_bytes(),
                [
                    "220 test.server.com Service ready\r\n",
                    "554 permanent problems with the remote server\r\n"
                ]
                .concat()
                .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        async fn file3() {
            assert!(make_test::<Test>(
                ["HELO foo.com\r\n"].concat().as_bytes(),
                ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        run_engine_test(
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            regex,
        )
        .await;

        run_engine_test(
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            regex2,
        )
        .await;

        run_engine_test(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            file,
        )
        .await;

        run_engine_test(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            file2,
        )
        .await;

        run_engine_test(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            file3,
        )
        .await;
    }
}
