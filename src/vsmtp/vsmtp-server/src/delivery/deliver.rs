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
    delivery::{add_trace_information, send_mail, SenderOutcome},
    log_channels,
    receiver::MailHandlerError,
    ProcessMessage,
};
use vsmtp_common::{
    queue::Queue,
    queue_path,
    re::{
        anyhow::{self, Context},
        log,
    },
    state::StateSMTP,
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{create_app_folder, Config, Resolvers};
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

pub async fn flush_deliver_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    log::info!(target: log_channels::DELIVERY, "Flushing deliver queue");

    let dir_entries =
        std::fs::read_dir(queue_path!(&config.server.queues.dirpath, Queue::Deliver))?;
    for path in dir_entries {
        let process_message = ProcessMessage {
            message_id: path?.path().file_name().unwrap().to_string_lossy().into(),
        };
        handle_one_in_delivery_queue(
            config.clone(),
            resolvers.clone(),
            process_message,
            rule_engine.clone(),
        )
        .await;
    }

    Ok(())
}

/// handle and send one email pulled from the delivery queue.
///
/// # Args
/// * `config` - the server's config.
/// * `resolvers` - a list of dns with their associated domains.
/// * `path` - the path to the message file.
/// * `rule_engine` - an instance of the rule engine.
///
/// # Errors
/// * failed to open the email.
/// * failed to parse the email.
/// * failed to send an email.
/// * rule engine mutex is poisoned.
/// * failed to add trace data to the email.
/// * failed to copy the email to other queues or remove it from the delivery queue.
///
/// # Panics
pub async fn handle_one_in_delivery_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) {
    log::info!(
        target: log_channels::DELIVERY,
        "handling message in delivery queue {}",
        process_message.message_id
    );

    if let Err(e) =
        handle_one_in_delivery_queue_inner(config, resolvers, process_message, rule_engine).await
    {
        log::warn!(
            target: log_channels::DELIVERY,
            "failed to handle one email in delivery queue: {}",
            e
        );
    }
}

async fn handle_one_in_delivery_queue_inner(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    log::debug!(
        target: log_channels::DELIVERY,
        "processing email '{}'",
        process_message.message_id
    );

    let (mail_context, mail_message) = Queue::Deliver
        .read(&config.server.queues.dirpath, &process_message.message_id)
        .await?;

    let (mut mail_context, mut mail_message, result) = RuleState::just_run_when(
        &StateSMTP::Delivery,
        config.as_ref(),
        resolvers.clone(),
        &rule_engine,
        mail_context,
        mail_message,
    )?;

    add_trace_information(&config, &mail_context, &mut mail_message, &result)?;

    match result {
        Status::Quarantine(path) => {
            let mut path = create_app_folder(&config, Some(&path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &mail_context)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            log::warn!(
                target: log_channels::DELIVERY,
                "delivery skipped due to quarantine."
            );

            // after processing the email is removed from the delivery queue.
            Queue::Deliver.remove(&config.server.queues.dirpath, &process_message.message_id)?;

            Ok(())
        }
        Status::Deny(_) => {
            // we update rcpt email status and write to dead queue in case of a deny.
            for rcpt in &mut mail_context.envelop.rcpt {
                rcpt.email_status = EmailTransferStatus::Failed {
                    timestamp: std::time::SystemTime::now(),
                    reason: "rule engine denied the email.".to_string(),
                };
            }
            Queue::Dead.write_to_queue(&config.server.queues.dirpath, &mail_context)?;

            log::warn!(
                target: log_channels::DELIVERY,
                "mail has been denied, moved to `dead` queue."
            );

            // after processing the email is removed from the delivery queue.
            Queue::Deliver.remove(&config.server.queues.dirpath, &process_message.message_id)?;

            Ok(())
        }
        _ => {
            match send_mail(&config, &mut mail_context, &mail_message, &resolvers).await {
                SenderOutcome::MoveToDead => {
                    Queue::Deliver
                        .move_to(&Queue::Dead, &config.server.queues.dirpath, &mail_context)
                        .with_context(|| {
                            format!(
                                "cannot move file from `{}` to `{}`",
                                Queue::Deliver,
                                Queue::Dead
                            )
                        })?;
                }
                SenderOutcome::MoveToDeferred => {
                    Queue::Deliver
                        .move_to(
                            &Queue::Deferred,
                            &config.server.queues.dirpath,
                            &mail_context,
                        )
                        .with_context(|| {
                            format!(
                                "cannot move file from `{}` to `{}`",
                                Queue::Deliver,
                                Queue::Deferred
                            )
                        })?;
                }
                SenderOutcome::RemoveFromDisk => {
                    Queue::Deliver
                        .remove(&config.server.queues.dirpath, &process_message.message_id)?;
                }
            }

            Ok(())
        }
    }
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
    use vsmtp_rule_engine::rule_engine::RuleEngine;
    use vsmtp_test::config;

    #[tokio::test]
    async fn basic() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();

        let now = std::time::SystemTime::now();

        Queue::Deliver
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
                        message_id: "message_from_deliver_to_deferred".to_string(),
                        skipped: None,
                    }),
                },
            )
            .unwrap();

        Queue::write_to_mails(
            &config.server.queues.dirpath,
            "message_from_deliver_to_deferred",
            &MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string()],
                body: Some("Hello world".to_string()),
            },
        )
        .unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::from_script(&config, "#{}").unwrap(),
        ));

        let resolvers = std::sync::Arc::new(build_resolvers(&config).unwrap());

        handle_one_in_delivery_queue(
            std::sync::Arc::new(config.clone()),
            resolvers,
            ProcessMessage {
                message_id: "message_from_deliver_to_deferred".to_string(),
            },
            rule_engine,
        )
        .await;

        std::fs::remove_file(queue_path!(
            &config.server.queues.dirpath,
            Queue::Deferred,
            "message_from_deliver_to_deferred"
        ))
        .unwrap();
    }
}
