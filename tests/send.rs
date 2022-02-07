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
const MTA_PORT_PLAIN: u16 = 10025;
// NOTE: todo submission port too (plain with auth)
const MTA_PORT_SUBMISSIONS: u16 = 10465;

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

#[test]
#[ignore = "require a server running"]
fn send_mail_plain() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .port(MTA_PORT_PLAIN)
        .build();

    lettre::Transport::send(&mailer, &email).unwrap();
}

#[test]
#[ignore = "require a server running and supporting tls"]
fn send_mail_starttls() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .tls(lettre::transport::smtp::client::Tls::Required(
            lettre::transport::smtp::client::TlsParameters::builder("example.com".to_string())
                .dangerous_accept_invalid_certs(true)
                .build()
                .unwrap(),
        ))
        .port(MTA_PORT_PLAIN)
        .build();

    lettre::Transport::send(&mailer, &email).unwrap();
}

#[test]
#[ignore = "require a server running and supporting tunneled tls"]
fn send_mail_tunneled_tls() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .tls(lettre::transport::smtp::client::Tls::Wrapper(
            lettre::transport::smtp::client::TlsParameters::builder("example.com".to_string())
                .dangerous_accept_invalid_certs(true)
                .build()
                .unwrap(),
        ))
        .port(MTA_PORT_SUBMISSIONS)
        .build();

    lettre::Transport::send(&mailer, &email).unwrap();
}
