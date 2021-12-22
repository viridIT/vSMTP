#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::{address::Address, tests::helpers::run_integration_engine_test},
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

    // -- testing out rcpt checking.

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

    // -- testing out rcpt actions.

    struct TestRcptAdded;

    #[async_trait::async_trait]
    impl DataEndResolver for TestRcptAdded {
        async fn on_data_end(
            _: &ServerConfig,
            ctx: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("johndoe@personnal.com").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("me@personnal.com").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("green@personnal.com").unwrap())
                .is_some());

            assert_eq!(ctx.envelop.rcpt.len(), 3);
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_add_rcpt() {
        assert!(run_integration_engine_test::<TestRcptAdded>(
            "./src/rules/tests/rules/rcpt/add_rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<johndoe@personnal.com>\r\n",
                "RCPT TO:<green@personnal.com>\r\n",
                "DATA\r\n",
                ".\r\n",
                "QUIT\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    struct TestRcptRemoved;

    #[async_trait::async_trait]
    impl DataEndResolver for TestRcptRemoved {
        async fn on_data_end(
            _: &ServerConfig,
            ctx: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            println!("{:?}", ctx.envelop.rcpt);

            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("staff@viridit.com").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("green@satan.org").unwrap())
                .is_none());

            assert_eq!(ctx.envelop.rcpt.len(), 1);
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_remove_rcpt() {
        assert!(run_integration_engine_test::<TestRcptRemoved>(
            "./src/rules/tests/rules/rcpt/rm_rcpt.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<staff@viridit.com>\r\n",
                "RCPT TO:<green@satan.org>\r\n",
                "DATA\r\n",
                ".\r\n",
                "QUIT\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }
}
