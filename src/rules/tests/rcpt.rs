#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        receiver::test_helpers::DefaultResolverTest,
        resolver::Resolver,
        rules::{address::Address, tests::helpers::run_integration_engine_test},
        smtp::mail::{Body, MailContext},
    };

    // -- testing out rcpt checking.

    #[tokio::test]
    async fn test_rcpt_by_user() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
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

        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
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
        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
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

        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
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
        assert!(run_integration_engine_test::<DefaultResolverTest>(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/rcpt.vsl",
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

    #[tokio::test]
    async fn test_rcpt_in_preq() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/contains_rcpt.vsl",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<johndoe@other.com>\r\n",
                "RCPT TO:<worker@viridit.com>\r\n",
                "RCPT TO:<customer@company.com>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/rcpt/contains_rcpt.vsl",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<johndoe@other.com>\r\n",
                "RCPT TO:<worker@viridit.com>\r\n",
                "RCPT TO:<customer@company.com>\r\n",
                "RCPT TO:<green@foo.com>\r\n",
                "DATA\r\n",
                "from: test <test@viridit.com>\r\n",
                "Subject: ...\r\n",
                "To: johndoe@personal.com, green@personal.com\r\n",
                "Message-ID: <xxx@localhost.com>\r\n",
                "Date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "...\r\n",
                ".\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok())
    }

    // -- testing out rcpt actions.

    struct TestRcptAdded;

    #[async_trait::async_trait]
    impl Resolver for TestRcptAdded {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("johndoe@personal.com").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("me@personal.com").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("green@personal.com").unwrap())
                .is_some());

            assert_eq!(ctx.envelop.rcpt.len(), 3);

            assert!(if let Body::Parsed(body) = &ctx.body {
                if let Some((_, to)) = body.headers.iter().find(|(header, _)| header == "to") {
                    to == "johndoe@personal.com, green@personal.com, me@personal.com"
                } else {
                    false
                }
            } else {
                false
            });

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_add_rcpt() {
        assert!(run_integration_engine_test::<TestRcptAdded>(
            "127.0.0.1:0",
            TestRcptAdded {},
            "./src/rules/tests/rules/rcpt/add_rcpt.vsl",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<johndoe@personal.com>\r\n",
                "RCPT TO:<green@personal.com>\r\n",
                "DATA\r\n",
                "from: test <test@viridit.com>\r\n",
                "Subject: ADD_RCPT\r\n",
                "To: johndoe@personal.com, green@personal.com\r\n",
                "Message-ID: <xxx@localhost.com>\r\n",
                "Date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "added rcpts!\r\n",
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
    impl Resolver for TestRcptRemoved {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
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
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("john@foo.eu").unwrap())
                .is_some());

            assert_eq!(ctx.envelop.rcpt.len(), 2);

            assert!(if let Body::Parsed(body) = &ctx.body {
                if let Some((_, to)) = body.headers.iter().find(|(header, _)| header == "to") {
                    to == "staff@viridit.com, john@foo.eu"
                } else {
                    false
                }
            } else {
                false
            });

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_remove_rcpt() {
        assert!(run_integration_engine_test::<TestRcptRemoved>(
            "127.0.0.1:0",
            TestRcptRemoved {},
            "./src/rules/tests/rules/rcpt/rm_rcpt.vsl",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<staff@viridit.com>\r\n",
                "RCPT TO:<green@satan.org>\r\n",
                "RCPT TO:<john@foo.eu>\r\n",
                "DATA\r\n",
                "from: test <test@viridit.com>\r\n",
                "Subject: DEL_RCPT\r\n",
                "To: staff@viridit.com, green@satan.org, john@foo.eu\r\n",
                "Message-ID: <xxx@localhost.com>\r\n",
                "Date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "Rewritten rcpts!\r\n",
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

    struct TestRcptRewritten;

    #[async_trait::async_trait]
    impl Resolver for TestRcptRewritten {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            println!("{:?}", ctx.envelop.rcpt);

            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("staff@viridit.fr").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("green@viridit.fr").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("john@viridit.fr").unwrap())
                .is_some());
            assert!(ctx
                .envelop
                .rcpt
                .get(&Address::new("other@unknown.eu").unwrap())
                .is_some());

            assert_eq!(ctx.envelop.rcpt.len(), 4);

            assert!(if let Body::Parsed(body) = &ctx.body {
                if let Some((_, to)) = body.headers.iter().find(|(header, _)| header == "to") {
                    to == "staff@viridit.fr, green@viridit.fr, john@viridit.fr, other@unknown.eu"
                } else {
                    false
                }
            } else {
                false
            });

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_rewrite_rcpt() {
        assert!(run_integration_engine_test::<TestRcptRewritten>(
            "127.0.0.1:0",
            TestRcptRewritten {},
            "./src/rules/tests/rules/rcpt/rw_rcpt.vsl",
            users::mock::MockUsers::with_current_uid(1),
            [
                "HELO foobar\r\n",
                "MAIL FROM:<test@viridit.com>\r\n",
                "RCPT TO:<staff@viridit.eu>\r\n",
                "RCPT TO:<green@viridit.org>\r\n",
                "RCPT TO:<john@viridit.com>\r\n",
                "RCPT TO:<other@unknown.eu>\r\n",
                "DATA\r\n",
                "from: test <test@viridit.com>\r\n",
                "Subject: RCPT\r\n",
                "To: staff@viridit.eu, green@viridit.org, john@viridit.com, other@unknown.eu\r\n",
                "Message-ID: <xxx@localhost.com>\r\n",
                "Date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "Rewritten rcpts!\r\n",
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
