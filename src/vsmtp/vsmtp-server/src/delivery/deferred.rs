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
    context_from_file_path, delivery::send_mail, log_channels, message_from_file_path,
    ProcessMessage,
};
use vsmtp_common::{
    queue::Queue,
    queue_path,
    re::{
        anyhow::{self, Context},
        log,
    },
    transfer::EmailTransferStatus,
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
    let (context_filepath, message_filepath) = (
        queue_path!(
            &config.server.queues.dirpath,
            Queue::Deferred,
            &process_message.message_id
        ),
        std::path::PathBuf::from_iter([
            config.server.queues.dirpath.clone(),
            "mails".into(),
            process_message.message_id.clone().into(),
        ]),
    );

    log::debug!(
        target: log_channels::DEFERRED,
        "processing email '{}'",
        process_message.message_id
    );

    let mut ctx = context_from_file_path(&context_filepath)
        .await
        .with_context(|| {
            format!(
                "failed to deserialize email in deferred queue '{}'",
                process_message.message_id
            )
        })?;

    let max_retry_deferred = config.server.queues.delivery.deferred_retry_max;
    let message = message_from_file_path(message_filepath).await?;

    send_mail(&config, &mut ctx, &message, &resolvers).await;

    // updating retry count, set status to Failed if threshold reached.
    for rcpt in &mut ctx.envelop.rcpt {
        match &mut rcpt.email_status {
            EmailTransferStatus::HeldBack(count) if *count >= max_retry_deferred => {
                rcpt.email_status = EmailTransferStatus::Failed(format!(
                    "maximum retry count of '{max_retry_deferred}' reached"
                ));
            }
            EmailTransferStatus::HeldBack(_) => {}
            _ => {
                // in the deferred queue, the email is considered as held back.
                rcpt.email_status = EmailTransferStatus::HeldBack(0);
            }
        };
    }

    if ctx
        .envelop
        .rcpt
        .iter()
        .any(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
    {
        // if there is still recipients left to send the email to, we just update the recipient list on disk.
        Queue::Deferred.write_to_queue(&config.server.queues.dirpath, &ctx)?;
    } else {
        // otherwise, we remove the file from the deferred queue.
        std::fs::remove_file(&context_filepath)?;
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
        transfer::{EmailTransferStatus, Transfer},
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
                                email_status: EmailTransferStatus::Waiting,
                            },
                            Rcpt {
                                address: addr!("to+2@client.com"),
                                transfer_method: Transfer::Maildir,
                                email_status: EmailTransferStatus::Waiting,
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
        let path = config.server.queues.dirpath.join("deferred/test_deferred");
        let msg = config.server.queues.dirpath.join("mails/test_deferred");

        handle_one_in_deferred_queue(
            std::sync::Arc::new(config),
            std::sync::Arc::new(resolvers),
            ProcessMessage {
                message_id: "test_deferred".to_string(),
            },
        )
        .await
        .unwrap();

        pretty_assertions::assert_eq!(
            context_from_file_path(&path).await.unwrap(),
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
                            email_status: EmailTransferStatus::HeldBack(1),
                        },
                        Rcpt {
                            address: addr!("to+2@client.com"),
                            transfer_method: Transfer::Maildir,
                            email_status: EmailTransferStatus::HeldBack(1),
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
            message_from_file_path(msg).await.unwrap(),
            MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string(),],
                body: Some("Hello world".to_string()),
            }
        );
    }
}
