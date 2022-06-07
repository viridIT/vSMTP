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
use vsmtp_common::{
    addr,
    mail_context::{MailContext, MessageBody},
    re::tokio,
    CodeID, {BodyType, Mail},
};
use vsmtp_mail_parser::MailMimeParser;
use vsmtp_server::Connection;
use vsmtp_server::OnMail;

use crate::test_receiver;

#[tokio::test]
async fn reset_helo() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            message: MessageBody,
        ) -> CodeID {
            let body = message.to_parsed::<MailMimeParser>().unwrap();

            assert_eq!(mail.envelop.helo, "foo");
            assert_eq!(mail.envelop.mail_from.full(), "a@b");
            assert_eq!(mail.envelop.rcpt, vec![addr!("b@c").into()]);
            assert_eq!(
                body,
                MessageBody::Parsed(Box::new(Mail {
                    headers: [
                        ("from", "a b <a@b>"),
                        ("date", "tue, 30 nov 2021 20:54:27 +0100"),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                    body: BodyType::Regular(vec!["mail content wow".to_string()])
                }))
            );

            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T,
        [
            "HELO foo\r\n",
            "RSET\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RCPT TO:<b@c>\r\n",
            "DATA\r\n",
            "from: a b <a@b>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            "\r\n",
            "mail content wow\r\n",
            ".\r\n"
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n"
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn reset_mail_from_error() {
    assert!(test_receiver! {
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "RCPT TO:<b@c>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn reset_mail_ok() {
    assert!(test_receiver! {
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "HELO foo2\r\n",
            "RCPT TO:<b@c>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn reset_rcpt_to_ok() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            message: MessageBody,
        ) -> CodeID {
            let body = message.to_parsed::<MailMimeParser>().unwrap();

            assert_eq!(mail.envelop.helo, "foo2");
            assert_eq!(mail.envelop.mail_from.full(), "d@e");
            assert_eq!(mail.envelop.rcpt, vec![addr!("b@c").into()]);
            assert_eq!(
                body,
                MessageBody::Parsed(Box::new(Mail {
                    headers: vec![],
                    body: BodyType::Undefined
                }))
            );
            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T,
        [
            "HELO foo\r\n",
            "MAIL FROM:<a@b>\r\n",
            "RSET\r\n",
            "HELO foo2\r\n",
            "MAIL FROM:<d@e>\r\n",
            "RCPT TO:<b@c>\r\n",
            "DATA\r\n",
            ".\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
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
    }
    .is_ok());
}

#[tokio::test]
async fn reset_rcpt_to_error() {
    assert!(test_receiver! {
        [
            "HELO foo\r\n",
            "MAIL FROM:<foo@foo>\r\n",
            "RCPT TO:<toto@bar>\r\n",
            "RSET\r\n",
            "RCPT TO:<toto2@bar>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "503 Bad sequence of commands\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn reset_rcpt_to_multiple_rcpt() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            mail: Box<MailContext>,
            message: MessageBody,
        ) -> CodeID {
            let body = message.to_parsed::<MailMimeParser>().unwrap();

            assert_eq!(mail.envelop.helo, "foo");
            assert_eq!(mail.envelop.mail_from.full(), "foo2@foo");
            assert_eq!(
                mail.envelop.rcpt,
                vec![addr!("toto2@bar").into(), addr!("toto3@bar").into()]
            );
            assert_eq!(
                body,
                MessageBody::Parsed(Box::new(Mail {
                    headers: [
                        ("from", "foo2 foo <foo2@foo>"),
                        ("date", "tue, 30 nov 2021 20:54:27 +0100"),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                    body: BodyType::Undefined
                }))
            );
            CodeID::Ok
        }
    }

    assert!(test_receiver! {
        on_mail => &mut T,
        [
            "HELO foo\r\n",
            "MAIL FROM:<foo@foo>\r\n",
            "RCPT TO:<toto@bar>\r\n",
            "RSET\r\n",
            "MAIL FROM:<foo2@foo>\r\n",
            "RCPT TO:<toto2@bar>\r\n",
            "RCPT TO:<toto3@bar>\r\n",
            "DATA\r\n",
            "from: foo2 foo <foo2@foo>\r\n",
            "date: tue, 30 nov 2021 20:54:27 +0100\r\n",
            ".\r\n"
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
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
    }
    .is_ok());
}
