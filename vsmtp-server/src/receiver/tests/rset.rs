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
    receiver::{Connection, OnMail},
    test_receiver,
};
use vsmtp_common::{
    address::Address,
    mail_context::{Body, MailContext},
};
use vsmtp_mail_parser::MailMimeParser;

#[tokio::test]
async fn reset_helo() {
    struct T;

    #[async_trait::async_trait]
    impl OnMail for T {
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            let body = match mail.body {
                Body::Empty => panic!("mail cannot be empty"),
                Body::Parsed(parsed) => parsed,
                Body::Raw(raw) => {
                    Box::new(MailMimeParser::default().parse(raw.as_bytes()).unwrap())
                }
            };

            assert_eq!(mail.envelop.helo, "foo");
            assert_eq!(mail.envelop.mail_from.full(), "a@b");
            assert_eq!(
                mail.envelop.rcpt,
                vec![Address::try_from("b@c".to_string()).unwrap().into()]
            );
            assert_eq!(body.headers.len(), 2);

            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
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
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            let body = match mail.body {
                Body::Empty => panic!("mail cannot be empty"),
                Body::Parsed(parsed) => parsed,
                Body::Raw(raw) => {
                    Box::new(MailMimeParser::default().parse(raw.as_bytes()).unwrap())
                }
            };

            assert_eq!(mail.envelop.helo, "foo2");
            assert_eq!(mail.envelop.mail_from.full(), "d@e");
            assert_eq!(
                mail.envelop.rcpt,
                vec![Address::try_from("b@c".to_string()).unwrap().into()]
            );
            assert!(body.headers.is_empty());

            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
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
        async fn on_mail<S: std::io::Read + std::io::Write + Send>(
            &mut self,
            conn: &mut Connection<'_, S>,
            mail: Box<MailContext>,
            _: &mut Option<String>,
        ) -> anyhow::Result<()> {
            let body = match mail.body {
                Body::Empty => panic!("mail cannot be empty"),
                Body::Parsed(parsed) => parsed,
                Body::Raw(raw) => {
                    Box::new(MailMimeParser::default().parse(raw.as_bytes()).unwrap())
                }
            };
            assert_eq!(mail.envelop.helo, "foo");
            assert_eq!(mail.envelop.mail_from.full(), "foo2@foo");
            assert_eq!(
                mail.envelop.rcpt,
                vec![
                    Address::try_from("toto2@bar".to_string()).unwrap().into(),
                    Address::try_from("toto3@bar".to_string()).unwrap().into()
                ]
            );
            assert_eq!(body.headers.len(), 2);
            conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
            Ok(())
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
