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
use crate::tests::tls::test_tls_tunneled;
use vsmtp_common::re::tokio;
use vsmtp_config::{get_rustls_config, ConfigServerVirtual, TlsSecurityLevel};

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn simple() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
        [
            "NOOP\r\n",
            "HELO client.com\r\n",
            "MAIL FROM:<foo@bar>\r\n",
            "RCPT TO:<bar@foo>\r\n",
            "DATA\r\n",
            ".\r\n",
            "QUIT\r\n",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
        [
            "220 testserver.com Service ready",
            "250 Ok",
            "250 Ok",
            "250 Ok",
            "250 Ok",
            "354 Start mail input; end with <CRLF>.<CRLF>",
            "250 Ok",
            "221 Service closing transmission channel",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
        20466,
        |config| {
            Some(std::sync::Arc::new(
                get_rustls_config(
                    config.server.tls.as_ref().unwrap(),
                    &config.server.r#virtual,
                )
                .unwrap(),
            ))
        },
        |_| None,
        |_| (),
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn starttls_under_tunnel() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
        ["NOOP\r\n", "STARTTLS\r\n", "QUIT\r\n"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        [
            "220 testserver.com Service ready",
            "250 Ok",
            "220 testserver.com Service ready",
            "554 5.5.1 Error: TLS already active",
            "221 Service closing transmission channel",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
        20467,
        |config| {
            Some(std::sync::Arc::new(
                get_rustls_config(
                    config.server.tls.as_ref().unwrap(),
                    &config.server.r#virtual,
                )
                .unwrap(),
            ))
        },
        |_| None,
        |_| (),
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn config_ill_formed() {
    let mut config = get_tls_config();
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;

    let (client, server) = test_tls_tunneled(
        "testserver.com",
        std::sync::Arc::new(config),
        vec!["NOOP\r\n".to_string()],
        vec![],
        20461,
        |_| None,
        |_| None,
        |_| (),
    )
    .await
    .unwrap();

    assert!(client.is_err());
    assert!(server.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn sni() {
    let mut config = get_tls_config();
    config.app.vsl.filepath = Some("./src/vsl/sni.vsl".into());
    config.server.tls.as_mut().unwrap().security_level = TlsSecurityLevel::Encrypt;
    config.server.r#virtual.insert(
        "second.testserver.com".to_string(),
        ConfigServerVirtual::with_tls(
            "src/template/certs/sni/second.certificate.crt",
            "src/template/certs/sni/second.private_key.rsa.key",
        )
        .unwrap(),
    );

    let (client, server) = test_tls_tunneled(
        "second.testserver.com",
        std::sync::Arc::new(config),
        ["NOOP\r\n", "QUIT\r\n"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        [
            "220 testserver.com Service ready",
            "250 Ok",
            "221 Service closing transmission channel",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
        20469,
        |config| {
            Some(std::sync::Arc::new(
                get_rustls_config(
                    config.server.tls.as_ref().unwrap(),
                    &config.server.r#virtual,
                )
                .unwrap(),
            ))
        },
        |_| None,
        |_| (),
    )
    .await
    .unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}
