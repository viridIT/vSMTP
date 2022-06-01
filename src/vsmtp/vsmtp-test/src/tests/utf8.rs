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
use crate::test_receiver;
use vsmtp_common::{
    addr,
    mail_context::{MailContext, MessageBody},
    re::tokio,
    CodeID, {BodyType, Mail},
};
use vsmtp_server::{Connection, OnMail};

macro_rules! test_lang {
    ($lang_code:expr) => {{
        struct T;

        #[async_trait::async_trait]
        impl OnMail for T {
            async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
                &mut self,
                _: &mut Connection<S>,
                mail: Box<MailContext>,
                message: MessageBody
            ) -> CodeID {
                assert_eq!(mail.envelop.helo, "foobar".to_string());
                assert_eq!(mail.envelop.mail_from.full(), "john@doe".to_string());
                assert_eq!(
                    mail.envelop.rcpt,
                    vec![addr!("aa@bb").into()]
                );

                let body = message
                    .to_parsed::<vsmtp_mail_parser::MailMimeParser>()
                    .unwrap();

                pretty_assertions::assert_eq!(
                    body,
                    MessageBody::Parsed(Box::new(Mail {
                        headers: [
                            ("from", "john doe <john@doe>"),
                            ("subject", "ar"),
                            ("to", "aa@bb"),
                            ("message-id", "<xxx@localhost.com>"),
                            ("date", "Tue, 30 Nov 2021 20:54:27 +0100"),
                        ]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                        body: BodyType::Regular(
                            include_str!($lang_code)
                                .lines()
                                .map(str::to_string)
                                .map(|s| if s.starts_with("..") {
                                    s[1..].to_string()
                                } else {
                                    s
                                })
                                .collect::<Vec<_>>()
                        )
                    }))
                );
                CodeID::Ok
            }
        }

        crate::test_receiver! {
            on_mail => &mut T,
            [
                "HELO foobar\r\n",
                "MAIL FROM:<john@doe>\r\n",
                "RCPT TO:<aa@bb>\r\n",
                "DATA\r\n",
                "from: john doe <john@doe>\r\n",
                "subject: ar\r\n",
                "to: aa@bb\r\n",
                "message-id: <xxx@localhost.com>\r\n",
                "date: Tue, 30 Nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                &include_str!($lang_code).lines().map(str::to_string).collect::<Vec<_>>().join("\r\n"),
                // adding a "\r\n" after the mail because [`join`] don t add after the final element
                "\r\n",
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
    }};
}

#[tokio::test]
async fn test_receiver_utf8_zh() {
    assert!(test_lang!("mail/zh.txt").is_ok());
}

#[tokio::test]
async fn test_receiver_utf8_el() {
    assert!(test_lang!("mail/el.txt").is_ok());
}

#[tokio::test]
async fn test_receiver_utf8_ar() {
    assert!(test_lang!("mail/ar.txt").is_ok());
}

#[tokio::test]
async fn test_receiver_utf8_ko() {
    assert!(test_lang!("mail/ko.txt").is_ok());
}
