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

    #[tokio::test]
    async fn test_mail_by_user() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM:<johndoe@test.com>\r\n",]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM:<unknown@test.com>\r\n",]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
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
    async fn test_mail_by_fqdn() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM:<johndoe@viridit.com>\r\n",]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM:<user@unknown.com>\r\n",]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
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
    async fn test_mail_by_address() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM:<customer@company.com>\r\n",]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/mail/mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<another.customer@company.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "554 permanent problems with the remote server\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    struct TestRewriten;

    #[async_trait::async_trait]
    impl DataEndResolver for TestRewriten {
        async fn on_data_end(
            _: &ServerConfig,
            ctx: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            println!("{:?}", ctx.envelop.rcpt);
            println!("{:?}", ctx.envelop.mail_from);

            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("client@other.com").unwrap())
                .is_some());
            assert_eq!(ctx.envelop.mail_from.full(), "no-reply@viridit.com");

            assert_eq!(ctx.envelop.rcpt.len(), 1);
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_mail_rewrite() {
        assert!(run_integration_engine_test::<TestRewriten>(
            "./src/rules/tests/rules/mail/rw_mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<steven@personnal.fr>\r\n",
                "RCPT TO:<client@other.com>\r\n",
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