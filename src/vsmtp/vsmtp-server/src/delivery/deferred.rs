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
use crate::{
    delivery::{send_mail, SenderOutcome},
    log_channels, ProcessMessage,
};
use vsmtp_common::{
    queue::Queue,
    queue_path,
    re::{
        anyhow::{self, Context},
        log,
    },
};
use vsmtp_config::{Config, Resolvers};

pub async fn flush_deferred_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
) -> anyhow::Result<()> {
    let dir_entries =
        std::fs::read_dir(queue_path!(&config.server.queues.dirpath, Queue::Deferred))?;
    for path in dir_entries {
        let process_message = ProcessMessage {
            message_id: path?.path().file_name().unwrap().to_string_lossy().into(),
            delegated: false,
        };

        if let Err(e) =
            handle_one_in_deferred_queue(config.clone(), resolvers.clone(), process_message).await
        {
            log::warn!(target: log_channels::DEFERRED, "{}", e);
        }
    }

    Ok(())
}

// NOTE: emails stored in the deferred queue are likely to slow down the process.
//       the pickup process of this queue should be slower than pulling from the delivery queue.
//       https://www.postfix.org/QSHAPE_README.html#queues
async fn handle_one_in_deferred_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
) -> anyhow::Result<()> {
    log::debug!(
        target: log_channels::DEFERRED,
        "processing email '{}'",
        process_message.message_id
    );

    let (mut mail_context, mail_message) = Queue::Deferred
        .read(&config.server.queues.dirpath, &process_message.message_id)
        .await?;

    match send_mail(&config, &mut mail_context, &mail_message, &resolvers).await {
        SenderOutcome::MoveToDead => {
            Queue::Deferred
                .move_to(&Queue::Dead, &config.server.queues.dirpath, &mail_context)
                .with_context(|| {
                    format!(
                        "cannot move file from `{}` to `{}`",
                        Queue::Deferred,
                        Queue::Dead
                    )
                })?;
        }
        SenderOutcome::MoveToDeferred => {
            Queue::Deferred
                .write_to_queue(&config.server.queues.dirpath, &mail_context)
                .with_context(|| format!("failed to update context in `{}`", Queue::Deferred))?;
        }
        SenderOutcome::RemoveFromDisk => {
            Queue::Deferred.remove(&config.server.queues.dirpath, &process_message.message_id)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsmtp_common::{
        addr,
        envelop::Envelop,
        mail_context::{ConnectionContext, MailContext, MessageBody, MessageMetadata},
        rcpt::Rcpt,
        re::tokio,
        transfer::{EmailTransferStatus, Transfer, TransferErrors},
    };
    use vsmtp_config::build_resolvers;
    use vsmtp_test::config;

    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn basic() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();
        config.app.vsl.filepath = Some("./src/tests/empty_main.vsl".into());

        let now = std::time::SystemTime::now();

        Queue::Deferred
            .write_to_queue(
                &config.server.queues.dirpath,
                &MailContext {
                    connection: ConnectionContext {
                        timestamp: now,
                        credentials: None,
                        is_authenticated: false,
                        is_secured: false,
                        server_name: "testserver.com".to_string(),
                        server_address: "127.0.0.1:25".parse().unwrap(),
                    },
                    client_addr: "127.0.0.1:80".parse().unwrap(),
                    envelop: Envelop {
                        helo: "client.com".to_string(),
                        mail_from: addr!("from@testserver.com"),
                        rcpt: vec![
                            Rcpt {
                                address: addr!("to+1@client.com"),
                                transfer_method: Transfer::Maildir,
                                email_status: EmailTransferStatus::Waiting {
                                    timestamp: std::time::SystemTime::now(),
                                },
                            },
                            Rcpt {
                                address: addr!("to+2@client.com"),
                                transfer_method: Transfer::Maildir,
                                email_status: EmailTransferStatus::Waiting {
                                    timestamp: std::time::SystemTime::now(),
                                },
                            },
                        ],
                    },
                    metadata: Some(MessageMetadata {
                        timestamp: now,
                        message_id: "test_deferred".to_string(),
                        skipped: None,
                    }),
                },
            )
            .unwrap();

        Queue::write_to_mails(
            &config.server.queues.dirpath,
            "test_deferred",
            &MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string()],
                body: Some("Hello world".to_string()),
            },
        )
        .unwrap();

        let resolvers = build_resolvers(&config).unwrap();

        handle_one_in_deferred_queue(
            std::sync::Arc::new(config.clone()),
            std::sync::Arc::new(resolvers),
            ProcessMessage {
                message_id: "test_deferred".to_string(),
                delegated: false,
            },
        )
        .await
        .unwrap();

        pretty_assertions::assert_eq!(
            Queue::Deferred
                .read_mail_context(&config.server.queues.dirpath, "test_deferred")
                .await
                .unwrap(),
            MailContext {
                connection: ConnectionContext {
                    timestamp: now,
                    credentials: None,
                    is_authenticated: false,
                    is_secured: false,
                    server_name: "testserver.com".to_string(),
                    server_address: "127.0.0.1:25".parse().unwrap(),
                },
                client_addr: "127.0.0.1:80".parse().unwrap(),
                envelop: Envelop {
                    helo: "client.com".to_string(),
                    mail_from: addr!("from@testserver.com"),
                    rcpt: vec![
                        Rcpt {
                            address: addr!("to+1@client.com"),
                            transfer_method: Transfer::Maildir,
                            email_status: EmailTransferStatus::HeldBack {
                                errors: vec![(
                                    std::time::SystemTime::now(),
                                    TransferErrors::NoSuchMailbox {
                                        name: "to+1".to_string()
                                    }
                                )]
                            },
                        },
                        Rcpt {
                            address: addr!("to+2@client.com"),
                            transfer_method: Transfer::Maildir,
                            email_status: EmailTransferStatus::HeldBack {
                                errors: vec![(
                                    std::time::SystemTime::now(),
                                    TransferErrors::NoSuchMailbox {
                                        name: "to+2".to_string()
                                    }
                                )]
                            },
                        },
                    ],
                },
                metadata: Some(MessageMetadata {
                    timestamp: now,
                    message_id: "test_deferred".to_string(),
                    skipped: None,
                }),
            }
        );
        pretty_assertions::assert_eq!(
            Queue::read_mail_message(&config.server.queues.dirpath, "test_deferred")
                .await
                .unwrap(),
            MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string(),],
                body: Some("Hello world".to_string()),
            }
        );
    }
}
