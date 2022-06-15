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
use super::get_tls_config;
use crate::{test_receiver, tests::tls::test_starttls};
use vsmtp_common::re::{anyhow, tokio};
use vsmtp_config::field::TlsSecurityLevel;

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() {
    let (client, server) = test_starttls(
        "testserver.com",
        std::sync::Arc::new(get_tls_config()),
        &["EHLO client.com\r\n", "STARTTLS\r\n"],
        &[
            "EHLO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<bar@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n",
        ],
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-STARTTLS",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "250 Ok",
            "250 Ok",
            "354 Start mail input; end with <CRLF>.<CRLF>",
            "250 Ok",
            "221 Service closing transmission channel",
        ],
        20027,
        true,
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn double_starttls() {
    let (client, server) = test_starttls(
        "testserver.com",
        std::sync::Arc::new(get_tls_config()),
        &["EHLO client.com\r\n", "STARTTLS\r\n"],
        &["EHLO secured.client.com\r\n", "STARTTLS\r\n", "QUIT\r\n"],
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-STARTTLS",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "220 testserver.com Service ready",
            "554 5.5.1 Error: TLS already active",
            "221 Service closing transmission channel",
        ],
        20037,
        true,
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test]
async fn test_receiver_7() {
    assert!(test_receiver! {
        [
            "EHLO foobar\r\n",
            "STARTTLS\r\n",
            "QUIT\r\n"
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "454 TLS not available due to temporary reason\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());
}

#[tokio::test]
async fn test_receiver_8() -> anyhow::Result<()> {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    assert!(test_receiver! {
        with_config => config,
        ["EHLO foobar\r\n", "MAIL FROM: <foo@bar>\r\n", "QUIT\r\n"]
            .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250-testserver.com\r\n",
            "250-STARTTLS\r\n",
            "250-8BITMIME\r\n",
            "250 SMTPUTF8\r\n",
            "530 Must issue a STARTTLS command first\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn config_ill_formed() {
    let (client, server) = test_starttls(
        "testserver.com",
        std::sync::Arc::new(get_tls_config()),
        &["EHLO client.com\r\n", "STARTTLS\r\n"],
        &["EHLO secured.client.com\r\n", "QUIT\r\n"],
        &[
            "220 testserver.com Service ready",
            "250-testserver.com",
            "250-STARTTLS",
            "250-8BITMIME",
            "250 SMTPUTF8",
            "220 testserver.com Service ready",
        ],
        20031,
        false,
    )
    .await
    .unwrap();

    assert!(client.is_err());
    assert!(server.is_err());
}
