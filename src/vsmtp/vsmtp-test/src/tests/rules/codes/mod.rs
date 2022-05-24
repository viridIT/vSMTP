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
use crate::{config, test_receiver, tests::auth::unsafe_auth_config};
use vsmtp_common::re::{base64, vsmtp_rsasl};
use vsmtp_server::auth;
use vsmtp_server::re::tokio;

#[tokio::test]
async fn info_message() {
    let mut config = config::local_test();
    config.app.vsl.filepath = Some(
        <std::path::PathBuf as std::str::FromStr>::from_str(
            "./src/tests/rules/codes/custom_codes_info.vsl",
        )
        .unwrap(),
    );

    assert!(test_receiver! {
        with_config => config,
        [
            "HELO someone\r\n",
            "HELO foo\r\n",
            "HELO bar\r\n",
            "HELO example.com\r\n",
            "MAIL FROM:<a@satan.org>\r\n",
            "MAIL FROM:<a@ok.org>\r\n",
            "RCPT TO:<b@ok.org>\r\n",
            "DATA\r\n",
            ".\r\n"
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 cannot identify with 'someone'.\r\n",
            "250 2.0.0 foo is not accepted as a helo value.\r\n",
            "250 I do not accept this email, sorry\r\n",
            "250 Ok\r\n",
            "250 satan.org is not valid, please try again.\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "500 I decided that you cannot send data.\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn deny_message() {
    let mut config = config::local_test();
    config.app.vsl.filepath = Some(
        <std::path::PathBuf as std::str::FromStr>::from_str(
            "./src/tests/rules/codes/custom_codes_deny.vsl",
        )
        .unwrap(),
    );

    assert!(test_receiver! {
        with_config => config.clone(),
        [
            "HELO someone\r\n",
            "MAIL FROM:<a@satan.org>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "501 4.7.1 satan is blacklisted on this server\r\n",
        ]
        .concat()
    }
    .is_ok());

    assert!(test_receiver! {
        with_config => config.clone(),
        [
            "HELO someone\r\n",
            "MAIL FROM:<a@evil.com>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "501 4.7.1 evil is blacklisted on this server\r\n",
        ]
        .concat()
    }
    .is_ok());

    assert!(test_receiver! {
        with_config => config,
        [
            "HELO someone\r\n",
            "MAIL FROM:<a@unpleasant.eu>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "501 4.7.1 unpleasant is blacklisted on this server\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn accept_message() {
    let mut config = unsafe_auth_config();
    config.app.vsl.filepath = Some(
        <std::path::PathBuf as std::str::FromStr>::from_str(
            "./src/tests/rules/codes/custom_codes_accept.vsl",
        )
        .unwrap(),
    );

    assert!(test_receiver! {
        with_auth => {
            let mut rsasl = vsmtp_rsasl::SASL::new().unwrap();
            rsasl.install_callback::<auth::Callback>();
            rsasl.store(Box::new(std::sync::Arc::new(config.clone())));
            rsasl
        },
        with_config => config.clone(),
        [
            "HELO client.com\r\n",
            &format!("AUTH PLAIN {}\r\n", base64::encode(format!("\0{}\0{}", "admin", "password"))),
            "MAIL FROM:<admin@company.com>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "235 2.7.0 Authentication succeeded\r\n",
            "250 welcome aboard chief\r\n"
        ]
        .concat()
    }
    .is_ok());
}
