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

use anyhow::Context;
use vsmtp_common::re::anyhow;
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::{
    auth, handle_connection, re::tokio, Connection, ConnectionKind, IoService, OnMail,
};

/// A type implementing Write+Read to emulate sockets
pub struct Mock<'a, T: std::io::Write + std::io::Read> {
    read_cursor: T,
    write_cursor: std::io::Cursor<&'a mut Vec<u8>>,
}

impl<'a, T: std::io::Write + std::io::Read> Mock<'a, T> {
    /// Create an new instance
    pub fn new(read: T, write: &'a mut Vec<u8>) -> Self {
        Self {
            read_cursor: read,
            write_cursor: std::io::Cursor::new(write),
        }
    }
}

impl<T: std::io::Write + std::io::Read> std::io::Write for Mock<'_, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_cursor.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.write_cursor.flush()
    }
}

impl<T: std::io::Write + std::io::Read> std::io::Read for Mock<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read_cursor.read(buf)
    }
}

/// used for testing, does not do anything once the email is received.
pub struct DefaultMailHandler;

#[async_trait::async_trait]
impl OnMail for DefaultMailHandler {
    async fn on_mail<S: std::io::Read + std::io::Write + Send>(
        &mut self,
        conn: &mut Connection<'_, S>,
        mail: Box<vsmtp_common::mail_context::MailContext>,
        helo_domain: &mut Option<String>,
    ) -> anyhow::Result<()> {
        *helo_domain = Some(mail.envelop.helo.clone());
        conn.send_code(vsmtp_common::code::SMTPReplyCode::Code250)?;
        Ok(())
    }
}

/// run a connection and assert output produced by vSMTP and @expected_output
///
/// # Errors
///
/// * the outcome of [`handle_connection`]
///
/// # Panics
///
/// * argument provided are ill-formed
pub async fn test_receiver_inner<M>(
    address: &str,
    mail_handler: &mut M,
    smtp_input: &[u8],
    expected_output: &[u8],
    config: std::sync::Arc<Config>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
) -> anyhow::Result<()>
where
    M: OnMail + Send,
{
    let mut written_data = Vec::new();
    let mut mock = Mock::new(std::io::Cursor::new(smtp_input.to_vec()), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::new(
        ConnectionKind::Opportunistic,
        address.parse().unwrap(),
        config.clone(),
        &mut io,
    );

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
        RuleEngine::new(&Some(config.app.vsl.filepath.clone()))
            .context("failed to initialize the engine")
            .unwrap(),
    ));

    let result = handle_connection(&mut conn, None, rsasl, rule_engine, mail_handler).await;
    std::io::Write::flush(&mut conn.io_stream.inner).unwrap();

    pretty_assertions::assert_eq!(
        std::str::from_utf8(expected_output),
        std::str::from_utf8(&written_data),
    );

    result
}

/// Call test_receiver_inner
#[macro_export]
macro_rules! test_receiver {
    ($input:expr, $output:expr) => {
        test_receiver! {
            on_mail => &mut $crate::receiver::DefaultMailHandler {},
            with_config => $crate::config::local_test(),
            $input,
            $output
        }
    };
    (on_mail => $resolver:expr, $input:expr, $output:expr) => {
        test_receiver! {
            on_mail => $resolver,
            with_config => $crate::config::local_test(),
            $input,
            $output
        }
    };
    (with_config => $config:expr, $input:expr, $output:expr) => {
        test_receiver! {
            on_mail => &mut $crate::receiver::DefaultMailHandler {},
            with_config => $config,
            $input,
            $output
        }
    };
    (on_mail => $resolver:expr, with_config => $config:expr, $input:expr, $output:expr) => {
        $crate::receiver::test_receiver_inner(
            "127.0.0.1:0",
            $resolver,
            $input.as_bytes(),
            $output.as_bytes(),
            std::sync::Arc::new($config),
            None,
        )
        .await
    };
    (with_auth => $auth:expr, with_config => $config:expr, $input:expr, $output:expr) => {
        test_receiver! {
            with_auth => $auth,
            with_config => $config,
            on_mail => &mut $crate::receiver::DefaultMailHandler {},
            $input,
            $output
        }
    };
    (with_auth => $auth:expr, with_config => $config:expr, on_mail => $resolver:expr, $input:expr, $output:expr) => {
        $crate::receiver::test_receiver_inner(
            "127.0.0.1:0",
            $resolver,
            $input.as_bytes(),
            $output.as_bytes(),
            std::sync::Arc::new($config),
            Some(std::sync::Arc::new(tokio::sync::Mutex::new($auth))),
        )
        .await
    };
}
