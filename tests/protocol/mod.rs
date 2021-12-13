#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Write};

    use log::LevelFilter;
    use vsmtp::{
        config::server_config::{
            InnerLogConfig, InnerRulesConfig, InnerSMTPConfig, InnerSMTPErrorConfig,
            InnerServerConfig, InnerTlsConfig, ServerConfig, TlsSecurityLevel,
        },
        mailprocessing::mail_receiver::{MailReceiver, State},
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
        ) -> (State, SMTPReplyCode) {
            // after a successful exchange, the server is ready for a new RCPT
            (State::MailFrom, SMTPReplyCode::Code250)
        }
    }

    fn get_test_config() -> ServerConfig {
        let mut config = ServerConfig {
            domain: "testserver.com".to_string(),
            version: "1.0.0".to_string(),
            server: InnerServerConfig {
                addr: "0.0.0.0:10025".parse().unwrap(),
            },
            log: InnerLogConfig {
                file: "./tests/generated/output.log".to_string(),
                level: HashMap::<String, LevelFilter>::new(),
            },
            tls: InnerTlsConfig {
                security_level: TlsSecurityLevel::None,
                capath: None,
                preempt_cipherlist: true,
                handshake_timeout: std::time::Duration::from_millis(10_000),
                sni_maps: None,
            },
            smtp: InnerSMTPConfig {
                spool_dir: "./tests/generated/spool/".to_string(),
                timeout_client: HashMap::<String, String>::new(),
                error: InnerSMTPErrorConfig {
                    soft_count: 5,
                    hard_count: 10,
                    delay: std::time::Duration::from_millis(100),
                },
                code: None,
            },
            rules: InnerRulesConfig {
                dir: String::default(),
            },
        };
        config.prepare();
        config
    }

    async fn make_test<T: vsmtp::resolver::DataEndResolver>(
        smtp_input: &[u8],
        expected_output: &[u8],
        config: ServerConfig,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> Result<(), std::io::Error> {
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
            ) -> (State, SMTPReplyCode) {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "jhon@doe");
                assert_eq!(ctx.envelop.rcpt, vec![Address::new("aa@bb").unwrap()]);
                assert_eq!(ctx.body, "");

                (State::MailFrom, SMTPReplyCode::Code250)
            }
        }

        assert!(make_test::<T>(
            [
                "HELO foobar\r\n",
                "MAIL FROM:<jhon@doe>\r\n",
                "RCPT TO:<aa@bb>\r\n",
                "DATA\r\n",
                ".\r\n",
                "QUIT\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
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
            ["MAIL FROM:<jhon@doe>\r\n"].concat().as_bytes(),
            [
                "220 {domain} Service ready\r\n",
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
            ["RCPT TO:<jhon@doe>\r\n"].concat().as_bytes(),
            [
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
                "250-{domain}\r\n",
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
                "220 {domain} Service ready\r\n",
                "250-{domain}\r\n",
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
                "MAIL FROM:<jhon@doe>\r\n",
                "EHLO\r\n",
                "EHLO\r\n",
                "HELP\r\n",
                "aieari\r\n",
                "not a valid smtp command\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
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
                "220 {domain} Service ready\r\n",
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
}
