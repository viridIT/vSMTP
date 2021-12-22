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
    async fn test_rcpt_by_user() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<johndoe@other.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<unknown-user@other.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "554 permanent problems with the remote server\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_rcpt_by_fqdn() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<staff@viridit.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<staff@unknown.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "554 permanent problems with the remote server\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_rcpt_by_address() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<customer@company.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }
}
