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
use crate::{context_from_file_path, delivery::send_email, log_channels, message_from_file_path};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    queue::Queue,
    queue_path,
    rcpt::Rcpt,
    re::{
        anyhow::{self, Context},
        log,
    },
    transfer::EmailTransferStatus,
};
use vsmtp_config::Config;

pub async fn flush_deferred_queue(
    config: &Config,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
) -> anyhow::Result<()> {
    let dir_entries =
        std::fs::read_dir(queue_path!(&config.server.queues.dirpath, Queue::Deferred))?;
    for path in dir_entries {
        if let Err(e) = handle_one_in_deferred_queue(config, resolvers, &path?.path()).await {
            log::warn!(target: log_channels::DEFERRED, "{}", e);
        }
    }

    Ok(())
}

// NOTE: emails stored in the deferred queue are likely to slow down the process.
//       the pickup process of this queue should be slower than pulling from the delivery queue.
//       https://www.postfix.org/QSHAPE_README.html#queues
async fn handle_one_in_deferred_queue(
    config: &Config,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::debug!(
        target: log_channels::DEFERRED,
        "processing email '{message_id}'"
    );

    let mut ctx = context_from_file_path(path).await.with_context(|| {
        format!("failed to deserialize email in deferred queue '{message_id}'",)
    })?;

    let max_retry_deferred = config.server.queues.delivery.deferred_retry_max;

    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("email metadata not available in deferred email"))?;

    let message_path = {
        let mut message_path = config.server.queues.dirpath.clone();
        message_path.push(format!("mails/{}", message_id));
        message_path
    };
    let message = message_from_file_path(message_path).await?;

    // TODO: at this point, only HeldBack recipients should be present in the queue.
    //       check if it is true or not.
    ctx.envelop.rcpt = send_email(
        config,
        resolvers,
        metadata,
        &ctx.envelop.mail_from,
        &ctx.envelop.rcpt,
        &message,
    )
    .await
    .context("failed to send emails from the deferred queue")?;

    // updating retry count, set status to Failed if threshold reached.
    ctx.envelop.rcpt = ctx
        .envelop
        .rcpt
        .into_iter()
        .map(|rcpt| Rcpt {
            email_status: match rcpt.email_status {
                EmailTransferStatus::HeldBack(count) if count >= max_retry_deferred => {
                    EmailTransferStatus::Failed(format!(
                        "maximum retry count of '{max_retry_deferred}' reached"
                    ))
                }
                EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count + 1),
                status => EmailTransferStatus::Failed(format!(
                    "wrong recipient status '{status}' found in the deferred queue"
                )),
            },
            ..rcpt
        })
        .collect();

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
        std::fs::remove_file(&path)?;
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
            &MessageBody::Raw(
                ["Date: bar", "From: foo", "", "Hello world"]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            ),
        )
        .unwrap();

        let resolvers = build_resolvers(&config).unwrap();

        handle_one_in_deferred_queue(
            &config,
            &resolvers,
            &config.server.queues.dirpath.join("deferred/test_deferred"),
        )
        .await
        .unwrap();

        pretty_assertions::assert_eq!(
            context_from_file_path(&config.server.queues.dirpath.join("deferred/test_deferred"))
                .await
                .unwrap(),
            MailContext {
                connection: ConnectionContext {
                    timestamp: now,
                    credentials: None,
                    is_authenticated: false,
                    is_secured: false,
                    server_name: "testserver.com".to_string(),
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
            message_from_file_path(config.server.queues.dirpath.join("mails/test_deferred"))
                .await
                .unwrap(),
            MessageBody::Raw(vec![
                "Date: bar".to_string(),
                "From: foo".to_string(),
                "".to_string(),
                "Hello world".to_string(),
            ])
        );
    }
}
