use crate::{
    config::server_config::ServerConfig,
    receiver::test_helpers::{test_receiver, DefaultResolverTest},
    resolver::Resolver,
    rules::address::Address,
    smtp::mail::{Body, MailContext},
};

fn get_regular_config() -> std::sync::Arc<ServerConfig> {
    std::sync::Arc::new(
        ServerConfig::builder()
            .with_server_default_port("test.server.com")
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/delivery", crate::collection! {})
            .with_rules("./tmp/nothing")
            .with_default_reply_codes()
            .build(),
    )
}

#[tokio::test]
async fn test_receiver_rset_1() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "foo");
            assert_eq!(ctx.envelop.mail_from.full(), "a@b");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from([Address::new("b@c").unwrap()])
            );
            assert!(match &ctx.body {
                Body::Parsed(body) => body.headers.is_empty(),
                _ => false,
            });

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T,
        [
            "HELO foo\r\n",
            "RSET\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RCPT TO:<b@c>\r\n",
            "DATA\r\n",
            "mail content wow\r\n",
            ".\r\n"
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
            "250 Ok\r\n"
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_rset_2() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "RCPT TO:<b@c>\r\n",
        ]
        .concat()
        .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_rset_3() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "HELO foo2\r\n",
            "RCPT TO:<b@c>\r\n",
        ]
        .concat()
        .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_rset_4() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "foo2");
            assert_eq!(ctx.envelop.mail_from.full(), "d@e");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from([Address::new("b@c").unwrap()])
            );
            assert!(match &ctx.body {
                Body::Parsed(body) => body.headers.is_empty(),
                _ => false,
            });

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T,
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "HELO foo2\r\n",
            "MAIL FROM:<d@e>\r\n",
            "RCPT TO:<b@c>\r\n",
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
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_rset_5() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "foo");
            assert_eq!(ctx.envelop.mail_from.full(), "foo@foo");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from([Address::new("toto@bar").unwrap()])
            );
            assert!(match &ctx.body {
                Body::Parsed(body) => body.headers.is_empty(),
                _ => false,
            });

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T,
        [
            "HELO foo\r\n",
            "MAIL FROM:<foo@foo>\r\n",
            "RCPT TO:<toto@bar>\r\n",
            "RSET\r\n",
            "RCPT TO:<toto2@bar>\r\n",
        ]
        .concat()
        .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_rset_6() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "foo");
            assert_eq!(ctx.envelop.mail_from.full(), "foo2@foo");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from([
                    Address::new("toto2@bar").unwrap(),
                    Address::new("toto3@bar").unwrap()
                ])
            );
            assert!(match &ctx.body {
                Body::Parsed(body) => body.headers.is_empty(),
                _ => false,
            });

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T,
        [
            "HELO foo\r\n",
            "MAIL FROM:<foo@foo>\r\n",
            "RCPT TO:<toto@bar>\r\n",
            "RSET\r\n",
            "MAIL FROM:<foo2@foo>\r\n",
            "RCPT TO:<toto2@bar>\r\n",
            "RCPT TO:<toto3@bar>\r\n",
            "DATA\r\n",
            ".\r\n"
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
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n"
        ]
        .concat()
        .as_bytes(),
        get_regular_config()
    )
    .await
    .is_ok());
}
