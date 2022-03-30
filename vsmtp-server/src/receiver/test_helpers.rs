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
use crate::{
    receiver::{
        connection::{Connection, ConnectionKind},
        handle_connection,
        io_service::IoService,
    },
    server::SaslBackend,
};
use anyhow::Context;
use vsmtp_common::re::anyhow;
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;

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
pub(crate) struct DefaultMailHandler;

#[async_trait::async_trait]
impl super::OnMail for DefaultMailHandler {
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

// TODO: we should use a ReceiverTestParameters struct
//       because their could be a lot of parameters to tweak for tests.
//       (the connection kind for example)
/// this function mocks all of the server's processes.
///
/// # Errors
///
/// # Panics
// #[deprecated]
pub async fn test_receiver_deprecated<M>(
    address: &str,
    mail_handler: &mut M,
    smtp_input: &[u8],
    expected_output: &[u8],
    config: std::sync::Arc<Config>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<SaslBackend>>>,
) -> anyhow::Result<()>
where
    M: super::OnMail + Send,
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
    std::io::Write::flush(&mut conn.io_stream.inner)?;

    pretty_assertions::assert_eq!(
        std::str::from_utf8(expected_output),
        std::str::from_utf8(&written_data),
    );

    result
}

#[cfg(test)]
pub(crate) fn get_regular_config() -> Config {
    Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name("testserver.com")
        .with_user_group_and_default_system("root", "root")
        .unwrap()
        .with_ipv4_localhost()
        .with_default_logs_settings()
        .with_spool_dir_and_default_queues("./tmp/delivery")
        .without_tls_support()
        .with_default_smtp_options()
        .with_default_smtp_error_handler()
        .with_default_smtp_codes()
        .with_default_app()
        .with_vsl("./src/receiver/tests/main.vsl")
        .with_default_app_logs()
        .without_services()
        .with_system_dns()
        .validate()
        .unwrap()
}

/// should only be on test
// #[cfg(test)]
#[macro_export]
macro_rules! test_receiver {
    ($input:expr, $output:expr) => {
        test_receiver! {
            on_mail => &mut $crate::receiver::test_helpers::DefaultMailHandler {},
            with_config => $crate::receiver::test_helpers::get_regular_config(),
            $input,
            $output
        }
    };
    (on_mail => $resolver:expr, $input:expr, $output:expr) => {
        test_receiver! {
            on_mail => $resolver,
            with_config => $crate::receiver::test_helpers::get_regular_config(),
            $input,
            $output
        }
    };
    (with_config => $config:expr, $input:expr, $output:expr) => {
        test_receiver! {
            on_mail => &mut $crate::receiver::test_helpers::DefaultMailHandler {},
            with_config => $config,
            $input,
            $output
        }
    };
    (on_mail => $resolver:expr, with_config => $config:expr, $input:expr, $output:expr) => {
        $crate::receiver::test_helpers::test_receiver_deprecated(
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
            on_mail => &mut $crate::receiver::test_helpers::DefaultMailHandler {},
            $input,
            $output
        }
    };
    (with_auth => $auth:expr, with_config => $config:expr, on_mail => $resolver:expr, $input:expr, $output:expr) => {
        $crate::receiver::test_helpers::test_receiver_deprecated(
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
