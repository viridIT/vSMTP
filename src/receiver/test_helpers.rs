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
use crate::{
    config::server_config::ServerConfig,
    processes::{
        delivery::handle_one_in_delivery_queue, mime::handle_one_in_working_queue, ProcessMessage,
    },
    queue::Queue,
    receiver::{
        connection::{Connection, ConnectionKind},
        handle_connection,
        io_service::IoService,
    },
    resolver::Resolver,
    rules::rule_engine::RuleEngine,
    smtp::mail::MailContext,
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

pub(crate) struct DefaultResolverTest;

#[async_trait::async_trait]
impl Resolver for DefaultResolverTest {
    async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
        Ok(())
    }
}

// TODO: should be a macro instead of a function.
//       also we should use a ReceiverTestParameters struct
//       because their could be a lot of parameters to tweak for tests.
//       (the connection kind for example)
/// this function mocks all of the server's processes.
pub async fn test_receiver<T>(
    address: &str,
    resolver: T,
    smtp_input: &[u8],
    expected_output: &[u8],
    config: std::sync::Arc<ServerConfig>,
) -> anyhow::Result<()>
where
    T: Resolver + Send + Sync + 'static,
{
    let mut written_data = Vec::new();
    let mut mock = Mock::new(std::io::Cursor::new(smtp_input.to_vec()), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = anyhow::Context::context(
        Connection::from_plain(
            ConnectionKind::Opportunistic,
            address.parse().unwrap(),
            config.clone(),
            &mut io,
        ),
        "failed to initialize connection",
    )
    .unwrap();

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
        anyhow::Context::context(
            RuleEngine::new(&config.rules.main_filepath.clone()),
            "failed to initialize the engine",
        )
        .unwrap(),
    ));

    let (working_sender, mut working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, mut delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let re_delivery = rule_engine.clone();
    let config_deliver = config.clone();

    let delivery_handle = tokio::spawn(async move {
        let mut resolvers =
            std::collections::HashMap::<String, Box<dyn Resolver + Send + Sync>>::new();
        resolvers.insert("default".to_string(), Box::new(resolver));

        while let Some(pm) = delivery_receiver.recv().await {
            handle_one_in_delivery_queue(
                &config_deliver,
                &std::path::PathBuf::from_iter([
                    Queue::Deliver
                        .to_path(&config_deliver.delivery.spool_dir)
                        .unwrap(),
                    std::path::Path::new(&pm.message_id).to_path_buf(),
                ]),
                &re_delivery,
                &mut resolvers,
            )
            .await
            .expect("delivery process failed");
        }
    });

    let re_mime = rule_engine.clone();
    let config_mime = config.clone();
    let mime_delivery_sender = delivery_sender.clone();
    let mime_handle = tokio::spawn(async move {
        while let Some(pm) = working_receiver.recv().await {
            handle_one_in_working_queue(&config_mime, &re_mime, pm, &mime_delivery_sender)
                .await
                .expect("mime process failed");
        }
    });

    let working_sender = std::sync::Arc::new(working_sender);
    let delivery_sender = std::sync::Arc::new(delivery_sender);

    handle_connection(
        &mut conn,
        None,
        rule_engine,
        working_sender,
        delivery_sender,
    )
    .await?;
    std::io::Write::flush(&mut conn.io_stream.inner)?;

    delivery_handle.await.unwrap();
    mime_handle.await.unwrap();

    // NOTE: could it be a good idea to remove the queue when all tests are done ?

    assert_eq!(
        std::str::from_utf8(&written_data),
        std::str::from_utf8(expected_output)
    );

    Ok(())
}

#[cfg(test)]
pub(crate) fn get_regular_config() -> anyhow::Result<ServerConfig> {
    ServerConfig::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_rfc_port("test.server.com", "foo", "foo", None)
        .without_log()
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/delivery", crate::collection! {})
        .with_rules("./src/receiver/tests/main.vsl", vec![])
        .with_default_reply_codes()
        .build()
}

#[cfg(test)]
pub mod logs {
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub fn setup_logs() {
        INIT.call_once(|| {
            let mut config = crate::receiver::test_helpers::get_regular_config()
                .expect("failed to create config for logs");
            config
                .log
                .level
                .insert("default".into(), log::LevelFilter::Warn);
            config.log.file =
                std::path::PathBuf::from_iter([".", "tests", "generated", "app.test.log"]);
            config.rules.logs.file =
                std::path::PathBuf::from_iter([".", "tests", "generated", "rules.test.log"]);

            log4rs::init_config(
                crate::config::get_logger_config(&config).expect("failed to init logs"),
            )
            .expect("failed to init logs");
        });
    }
}
