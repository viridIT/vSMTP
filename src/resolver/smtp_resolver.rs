/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
    config::server_config::ServerConfig,
    model::mail::{Body, MailContext},
    smtp::code::SMTPReplyCode,
};

use super::DataEndResolver;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Default)]
pub struct SMTPResolver;

#[async_trait::async_trait]
impl DataEndResolver for SMTPResolver {
    async fn on_data_end(
        &mut self,
        _: &ServerConfig,
        ctx: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        if let Body::Parsed(mail) = &ctx.body {
            let mut builder = Message::builder();
            for header in mail.headers.iter() {
                builder = match (header.0.as_str(), header.1.as_str()) {
                    ("from", value) => builder.from(value.parse().unwrap()),
                    ("to", value) => {
                        for inbox in value.split(", ") {
                            builder = builder.to(inbox.parse().unwrap())
                        }
                        builder
                    }
                    ("subject", value) => builder.subject(value),
                    _ => builder,
                };
            }

            let to_send = builder.body(mail.to_raw().1).unwrap();
            let mailer = SmtpTransport::builder_dangerous("127.0.0.1")
                .port(25)
                .build();

            match mailer.send(&to_send) {
                Ok(_) => log::debug!("email sent successfully."),
                Err(e) => log::error!("could not send email: {:?}", e),
            }
        } else {
            log::error!("email hasn't been parsed, exiting delivery ...");
        }

        Ok(SMTPReplyCode::Code250)
    }
}
