/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::{config, test_receiver};
use vsmtp_common::{
    addr,
    mail_context::MailContext,
    re::tokio,
    CodeID, MailHeaders, MessageBody, {BodyType, Mail},
};
use vsmtp_mail_parser::MailMimeParser;
use vsmtp_server::Connection;
use vsmtp_server::OnMail;

// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.3.2

#[tokio::test]
async fn test_receiver_1() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            _: MessageBody,
        ) -> CodeID {
            assert_eq!(mail.envelop.helo, "foobar");
            assert_eq!(mail.envelop.mail_from.full(), "john@doe");
            assert_eq!(mail.envelop.rcpt, vec![addr!("aa@bb").into()]);
            assert!(mail.metadata.is_some());
            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T,
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john@doe>\r\n",
            "RCPT TO:<aa@bb>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n",
        ]
        .concat(),
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
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_2() {
    assert!(test_receiver! {
        ["foo\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "501 Syntax error in parameters or arguments\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_3() {
    assert!(test_receiver! {
        ["MAIL FROM:<john@doe>\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_4() {
    assert!(test_receiver! {
        ["RCPT TO:<john@doe>\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_5() {
    assert!(test_receiver! {
        ["HELO foo\r\n", "RCPT TO:<bar@foo>\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_6() {
    assert!(test_receiver! {
        ["HELO foobar\r\n", "QUIT\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_10() {
    assert!(test_receiver! {
        ["HELP\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "214 joining us https://viridit.com/support\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_11() {
    assert!(test_receiver! {
        [
            "HELO postmaster\r\n",
            "MAIL FROM: <lala@foo>\r\n",
            "RCPT TO: <lala@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "DATA\r\n",
            "MAIL FROM:<b@b>\r\n",
        ]
        .concat(),
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
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_11_bis() {
    assert!(test_receiver! {
        [
            "HELO postmaster\r\n",
            "MAIL FROM: <lala@foo>\r\n",
            "RCPT TO: <lala@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "DATA\r\n",
            "RCPT TO:<b@b>\r\n",
        ]
        .concat(),
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
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_12() {
    let mut config = config::local_test();
    config.server.smtp.disable_ehlo = true;

    assert!(test_receiver! {
        with_config => config,
        ["EHLO postmaster\r\n"].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "502 Command not implemented\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn max_rcpt_reached() {
    let mut config = config::local_test();
    config.server.smtp.rcpt_count_max = 5;

    assert!(test_receiver! {
        with_config => config,
        [
            "EHLO client.com\r\n",
            "MAIL FROM:<foo@bar.com>\r\n",
            "RCPT TO:<foo+1@bar.com>\r\n",
            "RCPT TO:<foo+2@bar.com>\r\n",
            "RCPT TO:<foo+3@bar.com>\r\n",
            "RCPT TO:<foo+4@bar.com>\r\n",
            "RCPT TO:<foo+5@bar.com>\r\n",
            "RCPT TO:<foo+6@bar.com>\r\n",
        ].concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "452 Requested action not taken: too many recipients\r\n",
            "452 Requested action not taken: too many recipients\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_13() {
    struct T {
        count: u32,
    }

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            mut message: MessageBody,
        ) -> CodeID {
            assert_eq!(mail.envelop.helo, "foobar");
            assert_eq!(
                mail.envelop.mail_from.full(),
                format!("john{}@doe", self.count)
            );
            assert_eq!(
                mail.envelop.rcpt,
                vec![addr!(format!("aa{}@bb", self.count)).into()]
            );
            pretty_assertions::assert_eq!(
                *message.parsed::<MailMimeParser>().unwrap(),
                Mail {
                    headers: MailHeaders(
                        [
                            (
                                "from",
                                format!("john{} doe <john{}@doe>", self.count, self.count)
                            ),
                            ("date", "tue, 30 nov 2021 20:54:27 +0100".to_string()),
                        ]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect::<Vec<_>>()
                    ),
                    body: BodyType::Regular(vec![format!("mail {}", self.count)])
                }
            );

            self.count += 1;
            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T { count: 1 },
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john1@doe>\r\n",
            "RCPT TO:<aa1@bb>\r\n",
            "DATA\r\n",
            "from: john1 doe <john1@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "\r\n",
            "mail 1\r\n",
            ".\r\n",
            "MAIL FROM:<john2@doe>\r\n",
            "RCPT TO:<aa2@bb>\r\n",
            "DATA\r\n",
            "from: john2 doe <john2@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "\r\n",
            "mail 2\r\n",
            ".\r\n",
            "QUIT\r\n",
        ]
        .concat(),
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
    }
    .is_ok());
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn test_receiver_14() {
    struct T {
        count: u32,
    }

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            mut message: MessageBody,
        ) -> CodeID {
            assert_eq!(mail.envelop.helo, format!("foobar{}", self.count));
            assert_eq!(
                mail.envelop.mail_from.full(),
                format!("john{}@doe", self.count)
            );
            assert_eq!(
                mail.envelop.rcpt,
                vec![addr!(format!("aa{}@bb", self.count)).into()]
            );
            pretty_assertions::assert_eq!(
                *message.parsed::<MailMimeParser>().unwrap(),
                Mail {
                    headers: MailHeaders(
                        [
                            (
                                "from",
                                format!("john{} doe <john{}@doe>", self.count, self.count)
                            ),
                            ("date", "tue, 30 nov 2021 20:54:27 +0100".to_string()),
                        ]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect::<Vec<_>>()
                    ),
                    body: BodyType::Regular(vec![format!("mail {}", self.count)])
                }
            );

            self.count += 1;

            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T { count: 1 },
        [
            "HELO foobar1\r\n",
            "MAIL FROM:<john1@doe>\r\n",
            "RCPT TO:<aa1@bb>\r\n",
            "DATA\r\n",
            "from: john1 doe <john1@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "\r\n",
            "mail 1\r\n",
            ".\r\n",
            "HELO foobar2\r\n",
            "MAIL FROM:<john2@doe>\r\n",
            "RCPT TO:<aa2@bb>\r\n",
            "DATA\r\n",
            "from: john2 doe <john2@doe>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "\r\n",
            "mail 2\r\n",
            ".\r\n",
            "QUIT\r\n",
        ]
        .concat(),
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
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_9() {
    let mut config = config::local_test();
    config.server.smtp.error.delay = std::time::Duration::from_millis(100);
    config.server.smtp.error.soft_count = 5;
    config.server.smtp.error.hard_count = 10;

    let config = config;

    let before_test = std::time::Instant::now();
    assert!(test_receiver! {
        with_config => config.clone(),
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
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "503 Bad sequence of commands\r\n",
            "503 Bad sequence of commands\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "250 Ok\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "454 TLS not available due to temporary reason\r\n",
            "503 Bad sequence of commands\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "214 joining us https://viridit.com/support\r\n",
            "501 Syntax error in parameters or arguments\r\n",
            "451-Syntax error in parameters or arguments\r\n",
            "451 Too many errors from the client\r\n"
        ]
        .concat()
    }
    .is_err());

    assert!(
        before_test.elapsed().as_millis()
            >= config.server.smtp.error.delay.as_millis()
                * u128::try_from(
                    config.server.smtp.error.hard_count - config.server.smtp.error.soft_count
                )
                .unwrap()
    );
}
