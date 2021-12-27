#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig, model::mail::MailContext, resolver::DataEndResolver,
        rules::tests::helpers::run_integration_engine_test, smtp::code::SMTPReplyCode,
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
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.com\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO ibm.com\r\n"].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_types_helo() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.eu\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.com\r\n"].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.fr\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO green.foo\r\n"].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/helo/file_helo.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foo.com\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());
    }
}
