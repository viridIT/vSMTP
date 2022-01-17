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
    config::server_config::ServerConfig, model::mail::MailContext, smtp::code::SMTPReplyCode,
};

use super::DataEndResolver;
use lettre::{
    transport::smtp::authentication::Credentials,
    {Message, SmtpTransport, Transport},
};

#[derive(Default)]
pub struct SMTPResolver;

#[async_trait::async_trait]
impl DataEndResolver for SMTPResolver {
    async fn on_data_end(
        &mut self,
        _: &ServerConfig,
        _: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        let email = Message::builder()
            .from("NoBody <nobody@domain.tld>".parse().unwrap())
            .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
            .to("Hei <hei@domain.tld>".parse().unwrap())
            .subject("Happy new year")
            .body(String::from("Be happy!"))
            .unwrap();

        let creds = Credentials::new("smtp_username".to_string(), "smtp_password".to_string());

        // Open a remote connection to gmail
        let mailer = SmtpTransport::relay("smtp.gmail.com")
            .unwrap()
            .credentials(creds)
            .build();

        // Send the email
        match mailer.send(&email) {
            Ok(_) => println!("Email sent successfully!"),
            Err(e) => log::error!("Could not send email: {:?}", e),
        }

        Ok(SMTPReplyCode::Code250)
    }
}
