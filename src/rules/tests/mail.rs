#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        model::mail::{Body, MailContext},
        resolver::DataEndResolver,
        rules::{address::Address, tests::helpers::run_integration_engine_test},
        smtp::code::SMTPReplyCode,
        test_helpers::DefaultResolverTest,
    };

    #[tokio::test]
    async fn test_mail_by_user() {
        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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

        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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
        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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

        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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
        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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

        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
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

    struct TestRewritten;

    #[async_trait::async_trait]
    impl DataEndResolver for TestRewritten {
        async fn on_data_end(
            &mut self,
            _: &ServerConfig,
            ctx: &MailContext,
        ) -> anyhow::Result<SMTPReplyCode> {
            println!("{:?}", ctx.envelop.rcpt);
            println!("{:?}", ctx.envelop.mail_from);

            // envelop should have been rewritten.
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("client@other.com").unwrap())
                .is_some());
            assert_eq!(ctx.envelop.mail_from.full(), "no-reply@viridit.com");
            assert_eq!(ctx.envelop.rcpt.len(), 1);

            // the body of the email should have also been rewritten.
            assert!(if let Body::Parsed(body) = &ctx.body {
                if let Some((_, from)) = body.headers.iter().find(|(header, _)| header == "from") {
                    from.as_str() == "no-reply@viridit.com"
                } else {
                    false
                }
            } else {
                false
            });

            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_mail_rewrite() {
        assert!(run_integration_engine_test::<TestRewritten>(
            "127.0.0.1:0",
            TestRewritten {},
            "./src/rules/tests/rules/mail/rw_mail.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<steven@personal.fr>\r\n",
                "RCPT TO:<client@other.com>\r\n",
                "DATA\r\n",
                "from: steven personal <steven@personal.fr>\r\n",
                "Subject: text content\r\n",
                "To: client@other.com\r\n",
                "Message-ID: <xxx@localhost.com>\r\n",
                "Date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "A basic email.\r\n",
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
