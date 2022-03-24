use vsmtp_common::{address::Address, code::SMTPReplyCode, mail_context::MailContext, re::rsasl};
use vsmtp_config::{config::ConfigServerSMTPAuth, Config};

use crate::{receiver::test_helpers::get_regular_config, resolver::Resolver, test_receiver};

fn get_auth_config() -> Config {
    // TODO: make selection of SMTP extension and AUTH mechanism more simple

    let mut config = get_regular_config();
    config.server.smtp.codes.insert(
        SMTPReplyCode::Code250PlainEsmtp,
        [
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
        ]
        .concat(),
    );
    config
}

#[ignore]
#[tokio::test]
async fn auth() {
    let client =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("localhost")
            .authentication(vec![
                lettre::transport::smtp::authentication::Mechanism::Plain,
                lettre::transport::smtp::authentication::Mechanism::Login,
                lettre::transport::smtp::authentication::Mechanism::Xoauth2,
            ])
            .credentials(lettre::transport::smtp::authentication::Credentials::from(
                ("hello", "world"),
            ))
            .port(10015)
            .build::<lettre::Tokio1Executor>();

    lettre::AsyncTransport::send(
        &client,
        lettre::Message::builder()
            .from("NoBody <nobody@domain.tld>".parse().unwrap())
            .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
            .to("Hei <hei@domain.tld>".parse().unwrap())
            .subject("Happy new year")
            .body(String::from("Be happy!"))
            .unwrap(),
    )
    .await
    .unwrap();
}

struct TestAuth;

impl rsasl::Callback<(), ()> for TestAuth {
    fn callback(
        _sasl: &mut rsasl::SASL<(), ()>,
        session: &mut rsasl::Session<()>,
        prop: rsasl::Property,
    ) -> Result<(), rsasl::ReturnCode> {
        match prop {
            rsasl::Property::GSASL_VALIDATE_SIMPLE => {
                let (authcid, password) = (
                    session
                        .get_property(rsasl::Property::GSASL_AUTHID)
                        .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                        .to_str()
                        .unwrap()
                        .to_string(),
                    session
                        .get_property(rsasl::Property::GSASL_PASSWORD)
                        .ok_or(rsasl::ReturnCode::GSASL_NO_PASSWORD)?
                        .to_str()
                        .unwrap()
                        .to_string(),
                );

                if authcid == "hello" && password == "world" {
                    Ok(())
                } else {
                    Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR)
                }
            }
            _ => Err(rsasl::ReturnCode::GSASL_NO_CALLBACK),
        }
    }
}

#[tokio::test]
async fn plain_in_clair_secured() {
    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
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
            "250 STARTTLS\r\n"
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
            assert_eq!(ctx.envelop.helo, "authclient.com");
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
            "EHLO authclient.com\r\n",
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
async fn plain_in_clair_unsecured_bad_base64() {
    let mut config = get_auth_config();
    config.server.smtp.auth = Some(ConfigServerSMTPAuth {
        enable_dangerous_mechanism_in_clair: true,
        mechanisms: vec![],
    });

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<TestAuth>();
            rsasl
        },
        with_config => config,
        [
            "EHLO authclient.com\r\n",
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
async fn plain_in_clair_unsecured_without_initial_response() {
    struct T;

    #[async_trait::async_trait]
    impl Resolver for T {
        async fn deliver(&mut self, _: &Config, ctx: &MailContext) -> anyhow::Result<()> {
            assert_eq!(ctx.envelop.helo, "authclient.com");
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
            "EHLO authclient.com\r\n",
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
