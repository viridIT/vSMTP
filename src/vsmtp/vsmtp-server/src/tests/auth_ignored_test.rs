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
use vsmtp_common::re::tokio;

#[tokio::test]
#[ignore]
async fn test() {
    let sender =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("127.0.0.1")
            .port(10025)
            .authentication(vec![
                lettre::transport::smtp::authentication::Mechanism::Plain,
            ])
            .credentials(lettre::transport::smtp::authentication::Credentials::new(
                option_env!("AUTHID").unwrap().to_string(),
                option_env!("AUTHPASS").unwrap().to_string(),
            ))
            .build::<lettre::Tokio1Executor>();

    let email = lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap();

    lettre::AsyncTransport::send(&sender, email).await.unwrap();
}
