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
    connection::Connection,
    io_service::IoService,
    model::mail::MailContext,
    processes::{
        delivery::handle_one_in_delivery_queue, mime::handle_one_in_working_queue, ProcessMessage,
    },
    queue::Queue,
    resolver::Resolver,
    server::ServerVSMTP,
};

pub struct Mock<'a> {
    read_cursor: std::io::Cursor<Vec<u8>>,
    write_cursor: std::io::Cursor<&'a mut Vec<u8>>,
}

impl<'a> Mock<'a> {
    pub fn new(read: Vec<u8>, write: &'a mut Vec<u8>) -> Self {
        Self {
            read_cursor: std::io::Cursor::new(read),
            write_cursor: std::io::Cursor::new(write),
        }
    }
}

impl std::io::Write for Mock<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_cursor.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.write_cursor.flush()
    }
}

impl std::io::Read for Mock<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read_cursor.read(buf)
    }
}

pub struct DefaultResolverTest;

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
    let mut mock = Mock::new(smtp_input.to_vec(), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::<Mock<'_>>::from_plain(
        crate::connection::Kind::Opportunistic,
        address.parse().unwrap(),
        config.clone(),
        &mut io,
    )?;

    let (working_sender, mut working_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);
    let (delivery_sender, mut delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

    let config_deliver = config.clone();
    let deliver_handle = tokio::spawn(async move {
        let mut resolvers =
            std::collections::HashMap::<String, Box<dyn Resolver + Send + Sync>>::new();
        resolvers.insert("default".to_string(), Box::new(resolver));

        while let Some(pm) = delivery_receiver.recv().await {
            handle_one_in_delivery_queue(
                &mut resolvers,
                &std::path::PathBuf::from_iter([
                    Queue::Deliver
                        .to_path(&config_deliver.smtp.spool_dir)
                        .unwrap(),
                    std::path::Path::new(&pm.message_id).to_path_buf(),
                ]),
                &config_deliver,
            )
            .await
            .unwrap();
        }
    });

    let config_mime = config.clone();
    let from_mime = delivery_sender.clone();
    let mime_handle = tokio::spawn(async move {
        while let Some(pm) = working_receiver.recv().await {
            handle_one_in_working_queue(pm, &config_mime, &from_mime)
                .await
                .unwrap();
        }
    });

    ServerVSMTP::handle_connection::<Mock<'_>>(
        &mut conn,
        std::sync::Arc::new(working_sender),
        std::sync::Arc::new(delivery_sender),
        None,
    )
    .await?;
    std::io::Write::flush(&mut conn.io_stream.inner)?;

    mime_handle.await.unwrap();
    deliver_handle.await.unwrap();

    // NOTE: could it be a good idea to remove the queue when all tests are done ?

    assert_eq!(
        std::str::from_utf8(&written_data),
        std::str::from_utf8(&expected_output.to_vec())
    );

    Ok(())
}
