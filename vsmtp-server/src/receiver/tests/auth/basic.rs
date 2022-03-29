use vsmtp_common::{
    address::Address,
    mail_context::MailContext,
    re::{anyhow, rsasl},
};
use vsmtp_config::{Config, ConfigServerSMTPAuth};

use crate::{
    receiver::tests::auth::{get_auth_config, TestAuth},
    resolver::Resolver,
    test_receiver,
};

#[tokio::test]
async fn plain_in_clair_secured() {
    assert!(test_receiver! {
        with_auth => rsasl::SASL::new_untyped().unwrap(),
        with_config => get_auth_config(),
        [
            "EHLO foo\r\n",
            "AUTH PLAIN\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "538 5.7.11 Encryption required for requested authentication mechanism\r\n",
        ].concat()
    }
    .is_err());
}

#[tokio::test]
async fn plain_in_clair_unsecured() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &Config, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "client.com");
            assert_eq!(ctx.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from(
                    [Address::try_from("joe@doe".to_string()).unwrap()]
                )
            );

            Ok(())
        }
    }

    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        on_mail => T,
        [
            "EHLO client.com\r\n",
            &format!("AUTH PLAIN {}\r\n", base64::encode(format!("\0{}\0{}", "hello", "world"))),
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<joe@doe>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "235 2.7.0 Authentication succeeded\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n"
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn plain_in_clair_unsecured_utf8() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &Config, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "client.com");
            assert_eq!(ctx.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from(
                    [Address::try_from("joe@doe".to_string()).unwrap()]
                )
            );

            Ok(())
        }
    }

    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        on_mail => T,
        [
            "EHLO client.com\r\n",
            &format!("AUTH PLAIN {}\r\n", base64::encode(format!("\0{}\0{}", "héllo", "wÖrld"))),
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<joe@doe>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "235 2.7.0 Authentication succeeded\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n"
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn plain_in_clair_invalid_credentials() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        [
            "EHLO client.com\r\n",
            &format!("AUTH PLAIN {}\r\n", base64::encode(format!("\0{}\0{}", "foo", "bar"))),
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<joe@doe>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "535 5.7.8 Authentication credentials invalid\r\n"
        ].concat()
    }
    .is_err());
}

#[tokio::test]
async fn plain_in_clair_unsecured_cancel() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: 3,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        [
            "EHLO client.com\r\n",
            "AUTH PLAIN\r\n",
            "*\r\n",
            "AUTH PLAIN\r\n",
            "*\r\n",
            "AUTH PLAIN\r\n",
            "*\r\n",
            "AUTH PLAIN\r\n",
            "*\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "334 \r\n",
            "501 Authentication canceled by clients\r\n",
            "334 \r\n",
            "501 Authentication canceled by clients\r\n",
            "334 \r\n",
            "501 Authentication canceled by clients\r\n",
            "334 \r\n",
            "530 5.7.0 Authentication required\r\n"
        ].concat()
    }
    .is_err());
}

#[tokio::test]
async fn plain_in_clair_unsecured_bad_base64() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        [
            "EHLO client.com\r\n",
            "AUTH PLAIN foobar\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "501 5.5.2 Invalid, not base64\r\n",
            "503 Bad sequence of commands\r\n",
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn plain_in_clair_unsecured_without_initial_response() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &Config, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "client.com");
            assert_eq!(ctx.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                ctx.envelop.rcpt,
                std::collections::HashSet::from(
                    [Address::try_from("joe@doe".to_string()).unwrap()]
                )
            );

            Ok(())
        }
    }

    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        on_mail => T,
        [
            "EHLO client.com\r\n",
            "AUTH PLAIN\r\n",
            &format!("{}\r\n", base64::encode(format!("\0{}\0{}", "hello", "world"))),
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<joe@doe>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            // See https://datatracker.ietf.org/doc/html/rfc4422#section-5 2.a
            "334 \r\n",
            "235 2.7.0 Authentication succeeded\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n"
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn no_auth_with_authenticated_policy() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: true,
    });

    assert!(test_receiver! {
        with_config => config,
        [
            "EHLO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "530 5.7.0 Authentication required\r\n",
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn client_must_not_start() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
        attempt_count_max: -1,
        must_be_authenticated: false,
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        [
            "EHLO client.com\r\n",
            "AUTH LOGIN foobar\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
            "501 5.7.0 Client must not start with this mechanism\r\n"
        ].concat()
    }
    .is_err());
}
