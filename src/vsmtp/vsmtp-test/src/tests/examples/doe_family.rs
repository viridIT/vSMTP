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
*/
use crate::test_receiver;
use vsmtp_common::mail_context::MailContext;
use vsmtp_common::mail_context::MessageBody;
use vsmtp_common::CodeID;
use vsmtp_server::re::tokio;
use vsmtp_server::Connection;
use vsmtp_server::OnMail;

#[tokio::test]
async fn test_doe_family_setup() {
    #[derive(Clone)]
    struct MailHandler;

    #[async_trait::async_trait]
    impl OnMail for MailHandler {
        async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
            &mut self,
            _: &mut Connection<S>,
            ctx: Box<MailContext>,
            _: MessageBody,
        ) -> CodeID {
            ctx.envelop
                .rcpt
                .iter()
                .find(|rcpt| rcpt.address.full() == "jane.doe@doe-family.com")
                .unwrap();

            CodeID::Ok
        }
    }

    let toml = include_str!("../../../../../../examples/config/doe_family/vsmtp.toml");
    let config = vsmtp_config::Config::from_toml(toml).unwrap();

    assert!(test_receiver! {
        with_config => config.clone(),
        [
            "HELO example.com\r\n",
            "MAIL FROM:<a@spam-domain.org>\r\n",
        ]
        .concat(),
        [
            "220 doe-family.com Service ready\r\n",
            "250 Ok\r\n",
            "554 permanent problems with the remote server\r\n",
        ]
        .concat()
    }
    .is_ok());

    assert!(test_receiver! {
        on_mail => &mut MailHandler {},
        with_config => config,
        [
            "HELO example.com\r\n",
            "MAIL FROM:<a@example.com>\r\n",
            "RCPT TO:<jenny.doe@doe-family.com>\r\n",
            "RCPT TO:<somebody.else@example.com>\r\n",
            "DATA\r\n",
            "Date: Wed, 6 Dec 2000 05:55:00 -0800 (PST)\r\n",
            "From: a@example.com\r\n",
            "To: jenny.doe@doe-family.com, somebody.else@example.com\r\n",
            "Subject: Hi from France!\r\n",
            "\r\n",
            "Hey Jenny ! It's been a while since ....\r\n",
            ".\r\n"
        ]
        .concat(),
        [
            "220 doe-family.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
        ]
        .concat()
    }
    .is_ok());
}
