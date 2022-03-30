/*
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
use crate::receiver::{Connection, MailContext, OnMail};
use crate::test_receiver;
use vsmtp_common::address::Address;
use vsmtp_common::mail_context::Body;
use vsmtp_common::re::anyhow;

macro_rules! test_lang {
    ($lang_code:expr) => {{
        struct T;

        #[async_trait::async_trait]
        impl OnMail for T {
            async fn on_mail<S: std::io::Read + std::io::Write + Send>(
                &mut self,
                conn: &mut Connection<'_, S>,
                mail: Box<MailContext>,
                _: &mut Option<String>,
            ) -> anyhow::Result<()> {
                assert_eq!(mail.envelop.helo, "foobar".to_string());
                assert_eq!(mail.envelop.mail_from.full(), "john@doe".to_string());
                assert_eq!(
                    mail.envelop.rcpt,
                    vec![Address::try_from("aa@bb".to_string()).unwrap().into()]
                );

                let body = match mail.body {
                    Body::Empty => panic!("mail cannot be empty"),
                    Body::Parsed(parsed) => parsed,
                    Body::Raw(raw) => Box::new(
                        vsmtp_mail_parser::MailMimeParser::default()
                            .parse(raw.as_bytes())
                            .unwrap(),
                    ),
                };

                pretty_assertions::assert_eq!(
                    format!("{}\n", body.to_raw()).as_str(),
                    include_str!($lang_code)
                );

                conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
                Ok(())
            }
        }

        test_receiver! {
            on_mail => &mut T,
            [
                "HELO foobar\r\n",
                "MAIL FROM:<john@doe>\r\n",
                "RCPT TO:<aa@bb>\r\n",
                "DATA\r\n",
                include_str!($lang_code),
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
