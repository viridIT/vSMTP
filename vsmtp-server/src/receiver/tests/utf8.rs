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
use crate::{receiver::test_helpers::test_receiver, resolver::Resolver};
use vsmtp_common::{
    address::Address,
    mail_context::{Body, MailContext},
    rcpt::Rcpt,
};
use vsmtp_config::ServerConfig;

macro_rules! test_lang {
    ($lang_code:expr) => {{
        struct T;

        #[async_trait::async_trait]
        impl Resolver for T {
            async fn deliver(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
                _: &Rcpt,
            ) -> anyhow::Result<()> {
                assert_eq!(ctx.envelop.helo, "foobar".to_string());
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe".to_string());
                assert_eq!(
                    ctx.envelop.rcpt,
                    vec![Address::try_from("aa@bb").unwrap().into()]
                );
                assert!(match &ctx.body {
                    Body::Parsed(mail) => {
                        format!("{}\n", mail.to_raw()).as_str() == include_str!($lang_code)
                    }
                    _ => false,
                });

                Ok(())
            }
        }

        assert!(test_receiver(
            "127.0.0.1:0",
            T,
            [
                "HELO foobar\r\n",
                "MAIL FROM:<john@doe>\r\n",
                "RCPT TO:<aa@bb>\r\n",
                "DATA\r\n",
                include_str!($lang_code),
                ".\r\n",
                "QUIT\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
            std::sync::Arc::new(
                ServerConfig::builder()
                    .with_version_str("<1.0.0")
                    .unwrap()
                    .with_rfc_port("test.server.com", "root", "root", None)
                    .without_log()
                    .without_smtps()
                    .with_default_smtp()
                    .with_delivery("./tmp/delivery", "none")
                    .with_rules("./src/receiver/tests/main.vsl", vec![])
                    .with_default_reply_codes()
                    .build()?,
            )
        )
        .await
        .is_ok());
    }};
}

#[tokio::test]
async fn test_receiver_utf8_zh() -> anyhow::Result<()> {
    test_lang!("mail/zh.txt");
    Ok(())
}

#[tokio::test]
async fn test_receiver_utf8_el() -> anyhow::Result<()> {
    test_lang!("mail/el.txt");
    Ok(())
}

#[tokio::test]
async fn test_receiver_utf8_ar() -> anyhow::Result<()> {
    test_lang!("mail/ar.txt");
    Ok(())
}

#[tokio::test]
async fn test_receiver_utf8_ko() -> anyhow::Result<()> {
    test_lang!("mail/ko.txt");
    Ok(())
}
