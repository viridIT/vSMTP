#[cfg(test)]
mod tests {
    use std::io::Write;

    use vsmtp::{
        config::server_config::{InnerSMTPConfig, InnerTlsConfig, ServerConfig, TlsSecurityLevel},
        mailprocessing::mail_receiver::{MailReceiver, StateSMTP},
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::address::Address,
        smtp::code::SMTPReplyCode,
        tests::Mock,
    };

    // see https://datatracker.ietf.org/doc/html/rfc5321#section-4.3.2

    struct DefaultResolverTest;

    #[async_trait::async_trait]
    impl DataEndResolver for DefaultResolverTest {
        async fn on_data_end(
            _: &ServerConfig,
            _: usize,
            _: &MailContext,
        ) -> (StateSMTP, SMTPReplyCode) {
            // after a successful exchange, the server is ready for a new RCPT
            (StateSMTP::MailFrom, SMTPReplyCode::Code250)
        }
    }

    fn get_test_config() -> ServerConfig {
        toml::from_str(include_str!("config.toml")).expect("cannot parse config from toml")
    }

    async fn make_test<T: vsmtp::resolver::DataEndResolver>(
        smtp_input: &[u8],
        expected_output: &[u8],
        mut config: ServerConfig,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> Result<(), std::io::Error> {
        config.prepare();

        let mut receiver = MailReceiver::<T>::new(
            "0.0.0.0:0".parse().unwrap(),
            tls_config,
            std::sync::Arc::new(config),
        );
        let mut write = Vec::new();
        let mock = Mock::new(smtp_input.to_vec(), &mut write);

        match receiver.receive_plain(mock).await {
            Ok(mut mock) => {
                let _ = mock.flush();
                assert_eq!(
                    std::str::from_utf8(&write),
                    std::str::from_utf8(&expected_output.to_vec())
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[tokio::test]
    async fn test_receiver_1() {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                _: &ServerConfig,
                _: usize,
                ctx: &MailContext,
            ) -> (StateSMTP, SMTPReplyCode) {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                assert_eq!(ctx.envelop.rcpt, vec![Address::new("aa@bb").unwrap()]);
                assert_eq!(ctx.body, "");

                (StateSMTP::MailFrom, SMTPReplyCode::Code250)
            }
        }

        assert!(make_test::<T>(
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
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_2() {
        assert!(make_test::<DefaultResolverTest>(
            ["foo\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "501 Syntax error in parameters or arguments\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_3() {
        assert!(make_test::<DefaultResolverTest>(
            ["MAIL FROM:<john@doe>\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_4() {
        assert!(make_test::<DefaultResolverTest>(
            ["RCPT TO:<john@doe>\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_5() {
        assert!(make_test::<DefaultResolverTest>(
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
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_6() {
        assert!(make_test::<DefaultResolverTest>(
            ["HELO foobar\r\n", "QUIT\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_7() {
        assert!(make_test::<DefaultResolverTest>(
            ["EHLO foobar\r\n", "STARTTLS\r\n", "QUIT\r\n"]
                .concat()
                .as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "250-testserver.com\r\n",
                "250 STARTTLS\r\n",
                "454 TLS not available due to temporary reason\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::Encrypt,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_8() {
        assert!(make_test::<DefaultResolverTest>(
            ["EHLO foobar\r\n", "MAIL FROM: <foo@bar>\r\n", "QUIT\r\n"]
                .concat()
                .as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "250-testserver.com\r\n",
                "250 STARTTLS\r\n",
                "530 Must issue a STARTTLS command first\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::Encrypt,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_9() {
        let before_test = std::time::Instant::now();
        let res = make_test::<DefaultResolverTest>(
            [
                "RCPT TO:<bar@foo>\r\n",
                "MAIL FROM: <foo@bar>\r\n",
                "EHLO\r\n",
                "NOOP\r\n",
                "azeai\r\n",
                "STARTTLS\r\n",
                "MAIL FROM:<john@doe>\r\n",
                "EHLO\r\n",
                "EHLO\r\n",
                "HELP\r\n",
                "aieari\r\n",
                "not a valid smtp command\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "503 Bad sequence of commands\r\n",
                "503 Bad sequence of commands\r\n",
                "501 Syntax error in parameters or arguments\r\n",
                "250 Ok\r\n",
                "501 Syntax error in parameters or arguments\r\n",
                "503 Bad sequence of commands\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await;

        assert!(res.is_err());

        // (hard_error - soft_error) * error_delay
        assert!(before_test.elapsed().as_millis() >= 5 * 100);
    }

    #[tokio::test]
    async fn test_receiver_10() {
        assert!(make_test::<DefaultResolverTest>(
            ["HELP\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "214 joining us https://viridit.com/support\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::Encrypt,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_11() {
        assert!(make_test::<DefaultResolverTest>(
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
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                tls: InnerTlsConfig {
                    security_level: TlsSecurityLevel::None,
                    ..get_test_config().tls
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_12() {
        assert!(make_test::<DefaultResolverTest>(
            ["EHLO postmaster\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "502 Command not implemented\r\n",
            ]
            .concat()
            .as_bytes(),
            ServerConfig {
                smtp: InnerSMTPConfig {
                    disable_ehlo: true,
                    ..get_test_config().smtp
                },
                ..get_test_config()
            },
            None,
        )
        .await
        .is_ok());
    }
}
