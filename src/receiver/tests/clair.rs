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
use crate::{
    config::server_config::{ServerConfig, TlsSecurityLevel},
    receiver::test_helpers::{get_regular_config, test_receiver, DefaultResolverTest},
    resolver::Resolver,
    rules::address::Address,
    smtp::mail::{Body, MailContext},
};

// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.3.2

#[tokio::test]
async fn test_receiver_1() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "foobar");
            assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from([Address::new("aa@bb").unwrap()])
            );
            assert!(match &ctx.body {
                Body::Parsed(body) => body.headers.is_empty(),
                _ => false,
            });
            assert!(ctx.metadata.is_some());

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T,
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john@doe>\r\n",
            "RCPT TO:<aa@bb>\r\n",
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
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_2() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["foo\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "501 Syntax error in parameters or arguments\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_3() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["MAIL FROM:<john@doe>\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_4() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["RCPT TO:<john@doe>\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_5() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["HELO foo\r\n", "RCPT TO:<bar@foo>\r\n"]
            .concat()
            .as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_6() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["HELO foobar\r\n", "QUIT\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_10() {
    match test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["HELP\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "214 joining us https://viridit.com/support\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(
            ServerConfig::builder()
                .with_version_str("<1.0.0")
                .unwrap()
                .with_rfc_port("test.server.com", "root", "root", None)
                .without_log()
                .with_safe_default_smtps(TlsSecurityLevel::Encrypt, "dummy", "dummy", None)
                .with_default_smtp()
                .with_delivery("./tmp/delivery")
                .with_rules("./src/receiver/tests/main.vsl", vec![])
                .with_default_reply_codes()
                .build()
                .expect("could not build the server config"),
        ),
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            panic!("{}", err)
        }
    };
}

#[tokio::test]
async fn test_receiver_11() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        [
            "HELO postmaster\r\n",
            "MAIL FROM: <lala@foo>\r\n",
            "RCPT TO: <lala@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "DATA\r\n",
            "MAIL FROM:<b@b>\r\n",
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
            "503 Bad sequence of commands\r\n",
            "250 Ok\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_11_bis() {
    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        [
            "HELO postmaster\r\n",
            "MAIL FROM: <lala@foo>\r\n",
            "RCPT TO: <lala@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "DATA\r\n",
            "RCPT TO:<b@b>\r\n",
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
            "503 Bad sequence of commands\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_12() {
    let mut config = get_regular_config().unwrap();
    config.smtp.disable_ehlo = true;

    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["EHLO postmaster\r\n"].concat().as_bytes(),
        [
            "220 test.server.com Service ready\r\n",
            "502 Command not implemented\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(config)
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_13() {
    struct T {
        count: u32,
    }

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            match self.count {
                0 => {
                    assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                    assert_eq!(
                        ctx.envelop.rcpt,
                        std::collections::HashSet::from([Address::new("aa@bb").unwrap()])
                    );
                    assert!(match &ctx.body {
                        Body::Parsed(body) => body.headers.len() == 2,
                        _ => false,
                    });
                    assert!(ctx.metadata.is_some());
                }
                1 => {
                    assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(ctx.envelop.mail_from.full(), "john2@doe");
                    assert_eq!(
                        ctx.envelop.rcpt,
                        std::collections::HashSet::from([Address::new("aa2@bb").unwrap()])
                    );
                    assert!(match &ctx.body {
                        Body::Parsed(body) => body.headers.len() == 2,
                        _ => false,
                    });
                }
                _ => panic!(),
            }

            self.count += 1;

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T { count: 0 },
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john@doe>\r\n",
            "RCPT TO:<aa@bb>\r\n",
            "DATA\r\n",
            "from: john doe <john@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "mail one\r\n",
            ".\r\n",
            "MAIL FROM:<john2@doe>\r\n",
            "RCPT TO:<aa2@bb>\r\n",
            "DATA\r\n",
            "from: john2 doe <john2@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "mail two\r\n",
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
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_14() {
    struct T {
        count: u32,
    }

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
            match self.count {
                0 => {
                    assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                    assert_eq!(
                        ctx.envelop.rcpt,
                        std::collections::HashSet::from([Address::new("aa@bb").unwrap()])
                    );
                    assert!(match &ctx.body {
                        Body::Parsed(body) => body.headers.len() == 2,
                        _ => false,
                    });
                }
                1 => {
                    assert_eq!(ctx.envelop.helo, "foobar2");
                    assert_eq!(ctx.envelop.mail_from.full(), "john2@doe");
                    assert_eq!(
                        ctx.envelop.rcpt,
                        std::collections::HashSet::from([Address::new("aa2@bb").unwrap()])
                    );
                    assert!(match &ctx.body {
                        Body::Parsed(body) => body.headers.len() == 2,
                        _ => false,
                    });
                    assert!(ctx.metadata.is_some());
                }
                _ => panic!(),
            }

            self.count += 1;

            Ok(())
        }
    }

    assert!(test_receiver(
        "127.0.0.1:0",
        T { count: 0 },
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john@doe>\r\n",
            "RCPT TO:<aa@bb>\r\n",
            "DATA\r\n",
            "from: john doe <john@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "mail one\r\n",
            ".\r\n",
            "HELO foobar2\r\n",
            "MAIL FROM:<john2@doe>\r\n",
            "RCPT TO:<aa2@bb>\r\n",
            "DATA\r\n",
            "from: john2 doe <john2@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "mail two\r\n",
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
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config().unwrap()),
    )
    .await
    .is_ok());
}
