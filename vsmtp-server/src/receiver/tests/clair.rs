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
use crate::receiver::test_helpers::{get_regular_config, test_receiver, DefaultResolverTest};
use vsmtp_common::{address::Address, mail_context::MessageMetadata, rcpt::Rcpt};
use vsmtp_config::{config::ConfigServerTls, Config, TlsSecurityLevel};
use vsmtp_delivery::transport::Transport;

// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.3.2

#[tokio::test]
async fn test_receiver_1() {
    struct T;

    #[async_trait::async_trait]
    impl Transport for T {
        async fn deliver(
            &mut self,
            _: &Config,
            _: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            _: &str,
        ) -> anyhow::Result<()> {
            // assert_eq!(ctx.envelop.helo, "foobar");
            assert_eq!(from.full(), "john@doe");
            assert_eq!(
                to,
                vec![Address::try_from("aa@bb".to_string()).unwrap().into()]
            );
            // assert!(match &ctx.body {
            //     Body::Parsed(body) => body.headers.is_empty(),
            //     _ => false,
            // });
            // assert!(ctx.metadata.is_some());

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
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
            "501 Syntax error in parameters or arguments\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_10() {
    let mut config = get_regular_config();
    config.server.tls = Some(ConfigServerTls {
        security_level: TlsSecurityLevel::Encrypt,
        preempt_cipherlist: false,
        handshake_timeout: std::time::Duration::from_millis(200),
        protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
        certificate: rustls::Certificate(vec![]),
        private_key: rustls::PrivateKey(vec![]),
        sni: vec![],
    });

    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["HELP\r\n"].concat().as_bytes(),
        [
            "220 testserver.com Service ready\r\n",
            "214 joining us https://viridit.com/support\r\n",
        ]
        .concat()
        .as_bytes(),
        std::sync::Arc::new(get_regular_config())
    )
    .await
    .is_ok());
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
            "220 testserver.com Service ready\r\n",
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
        std::sync::Arc::new(get_regular_config()),
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
            "220 testserver.com Service ready\r\n",
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
        std::sync::Arc::new(get_regular_config()),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_receiver_12() {
    let mut config = get_regular_config();
    config.server.smtp.disable_ehlo = true;

    assert!(test_receiver(
        "127.0.0.1:0",
        DefaultResolverTest,
        ["EHLO postmaster\r\n"].concat().as_bytes(),
        [
            "220 testserver.com Service ready\r\n",
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
    impl Transport for T {
        async fn deliver(
            &mut self,
            _: &Config,
            _: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            _: &str,
        ) -> anyhow::Result<()> {
            match self.count {
                0 => {
                    // assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(from.full(), "john@doe");
                    assert_eq!(
                        to,
                        vec![vsmtp_common::rcpt::Rcpt::new(
                            Address::try_from("aa@bb".to_string()).unwrap()
                        )]
                    );
                    // assert!(match &ctx.body {
                    //     Body::Parsed(body) => body.headers.len() == 2,
                    //     _ => false,
                    // });
                    // assert!(ctx.metadata.is_some());
                }
                1 => {
                    // assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(from.full(), "john2@doe");
                    assert_eq!(
                        to,
                        vec![Address::try_from("aa2@bb".to_string()).unwrap().into()]
                    );
                    // assert!(match &ctx.body {
                    //     Body::Parsed(body) => body.headers.len() == 2,
                    //     _ => false,
                    // });
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
            "220 testserver.com Service ready\r\n",
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
        std::sync::Arc::new(get_regular_config()),
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
    impl Transport for T {
        async fn deliver(
            &mut self,
            _: &Config,
            _: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            _: &str,
        ) -> anyhow::Result<()> {
            match self.count {
                0 => {
                    // assert_eq!(ctx.envelop.helo, "foobar");
                    assert_eq!(from.full(), "john@doe");
                    assert_eq!(
                        to,
                        vec![Address::try_from("aa@bb".to_string()).unwrap().into()]
                    );
                    // assert!(match &ctx.body {
                    //     Body::Parsed(body) => body.headers.len() == 2,
                    //     _ => false,
                    // });
                }
                1 => {
                    // assert_eq!(ctx.envelop.helo, "foobar2");
                    assert_eq!(from.full(), "john2@doe");
                    assert_eq!(
                        to,
                        vec![Address::try_from("aa2@bb".to_string()).unwrap().into()]
                    );
                    // assert!(match &ctx.body {
                    //     Body::Parsed(body) => body.headers.len() == 2,
                    //     _ => false,
                    // });
                    // assert!(ctx.metadata.is_some());
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
            "220 testserver.com Service ready\r\n",
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
        std::sync::Arc::new(get_regular_config()),
    )
    .await
    .is_ok());
}
