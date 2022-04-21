use crate::{
    test_receiver,
    tests::auth::{safe_auth_config, unsafe_auth_config},
};
use vsmtp_common::{
    address::Address,
    mail_context::MailContext,
    re::{anyhow, base64, rsasl},
};
use vsmtp_server::re::tokio;
use vsmtp_server::Connection;
use vsmtp_server::{auth, OnMail};

#[tokio::test]
async fn plain_in_clair_secured() {
    assert!(test_receiver! {
        with_auth => rsasl::SASL::new_untyped().unwrap(),
        with_config => safe_auth_config(),
        [
            "EHLO foo\r\n",
            "AUTH PLAIN\r\n"
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-AUTH \r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "538 5.7.11 Encryption required for requested authentication mechanism\r\n",
        ].concat()
    }
    .is_err());
}

#[tokio::test]
async fn plain_in_clair_unsecured() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            assert_eq!(mail.envelop.helo, "client.com");
            assert_eq!(mail.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                mail.envelop.rcpt,
                vec![Address::try_from("joe@doe".to_string()).unwrap().into()]
            );

            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
        }
    }

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
        on_mail => &mut T,
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
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
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
    impl OnMail for T {
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            assert_eq!(mail.envelop.helo, "client.com");
            assert_eq!(mail.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                mail.envelop.rcpt,
                vec![Address::try_from("joe@doe".to_string()).unwrap().into()]
            );

            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
        }
    }

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
        on_mail => &mut T,
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
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
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
    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
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
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "535 5.7.8 Authentication credentials invalid\r\n"
        ].concat()
    }
    .is_err());
}

#[tokio::test]
async fn plain_in_clair_unsecured_cancel() {
    let mut config = unsafe_auth_config();
    config.server.smtp.auth.as_mut().unwrap().attempt_count_max = 3;

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
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
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
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
    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
        [
            "EHLO client.com\r\n",
            "AUTH PLAIN foobar\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
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
    impl OnMail for T {
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            assert_eq!(mail.envelop.helo, "client.com");
            assert_eq!(mail.envelop.mail_from.full(), "foo@bar");
            assert_eq!(
                mail.envelop.rcpt,
                vec![Address::try_from("joe@doe".to_string()).unwrap().into()]
            );

            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
        }
    }

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
        on_mail => &mut T,
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
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
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
    let mut config = unsafe_auth_config();
    config
        .server
        .smtp
        .auth
        .as_mut()
        .unwrap()
        .must_be_authenticated = true;

    assert!(test_receiver! {
        with_config => config,
        [
            "EHLO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "530 5.7.0 Authentication required\r\n",
        ].concat()
    }
    .is_ok());
}

#[tokio::test]
async fn client_must_not_start() {
    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = rsasl::SASL::new_untyped().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl
        },
        with_config => unsafe_auth_config(),
        [
            "EHLO client.com\r\n",
            "AUTH LOGIN foobar\r\n",
            "MAIL FROM:<foo@bar>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-AUTH PLAIN LOGIN CRAM-MD5\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "501 5.7.0 Client must not start with this mechanism\r\n"
        ].concat()
    }
    .is_err());
}
