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
    delegate,
    delivery::{add_trace_information, send_mail, SenderOutcome},
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
use vsmtp_rule_engine::{RuleEngine, RuleState};

pub async fn flush_deliver_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    rule_engine: std::sync::Arc<RuleEngine>,
) -> anyhow::Result<()> {
    log::info!("Flushing deliver queue");

    let dir_entries =
        std::fs::read_dir(queue_path!(&config.server.queues.dirpath, Queue::Deliver))?;
    for path in dir_entries {
        let process_message = ProcessMessage {
            message_id: path?.path().file_name().unwrap().to_string_lossy().into(),
            delegated: false,
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
/// # Errors
///
/// * failed to open the email.
/// * failed to parse the email.
/// * failed to send an email.
/// * rule engine mutex is poisoned.
/// * failed to add trace data to the email.
/// * failed to copy the email to other queues or remove it from the delivery queue.
pub async fn handle_one_in_delivery_queue(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    rule_engine: std::sync::Arc<RuleEngine>,
) {
    log::info!(
        "handling message in delivery queue {}",
        process_message.message_id
    );

    if let Err(e) =
        handle_one_in_delivery_queue_inner(config, resolvers, process_message, rule_engine).await
    {
        log::warn!("failed to handle one email in delivery queue: {e}");
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_one_in_delivery_queue_inner(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    rule_engine: std::sync::Arc<RuleEngine>,
) -> anyhow::Result<()> {
    log::debug!("processing email '{}'", process_message.message_id);

    let queue = if process_message.delegated {
        Queue::Delegated
    } else {
        Queue::Deliver
    };

    let (mail_context, mail_message) = queue
        .read(&config.server.queues.dirpath, &process_message.message_id)
        .await?;

    let (mut mail_context, mut mail_message, result, skipped) = RuleState::just_run_when(
        &StateSMTP::Delivery,
        config.as_ref(),
        resolvers.clone(),
        &rule_engine,
        mail_context,
        mail_message,
    );

    match &skipped {
        Some(Status::Quarantine(path)) => {
            let mut path = create_app_folder(&config, Some(path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &mail_context)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            // after processing the email is removed from the delivery queue.
            queue.remove(&config.server.queues.dirpath, &process_message.message_id)?;

            mail_message
                .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
                .map_err(MailHandlerError::WriteMessageBody)?;

            log::warn!("skipped due to quarantine.");

            return Ok(());
        }
        Some(Status::Delegated(delegator)) => {
            mail_context.metadata.as_mut().unwrap().skipped = Some(Status::DelegationResult);

            queue.move_to(
                &Queue::Delegated,
                &config.server.queues.dirpath,
                &mail_context,
            )?;

            mail_message
                .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
                .map_err(MailHandlerError::WriteMessageBody)?;

            // NOTE: needs to be executed after writing, because the other
            //       thread could pickup the email faster than this function.
            delegate(delegator, &mail_context, &mail_message)
                .map_err(MailHandlerError::DelegateMessage)?;

            log::warn!("skipped due to delegation.");

            return Ok(());
        }
        Some(Status::DelegationResult) => unreachable!(
            "delivery is the last stage, delegation results cannot travel down any further."
        ),
        Some(Status::Deny(code)) => {
            for rcpt in &mut mail_context.envelop.rcpt {
                rcpt.email_status = EmailTransferStatus::Failed {
                    timestamp: std::time::SystemTime::now(),
                    reason: format!("rule engine denied the message in delivery: {code:?}."),
                };
            }

            queue.move_to(&Queue::Dead, &config.server.queues.dirpath, &mail_context)?;

            mail_message
                .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
                .map_err(MailHandlerError::WriteMessageBody)?;

            log::warn!("mail has been denied and moved to the `dead` queue.");

            return Ok(());
        }
        Some(reason) => {
            log::warn!("skipped due to '{}'.", reason.as_ref());
        }
        None => {}
    };

    add_trace_information(&config, &mail_context, &mut mail_message, &result)?;

    match send_mail(&config, &mut mail_context, &mail_message, &resolvers).await {
        SenderOutcome::MoveToDead => {
            queue
                .move_to(&Queue::Dead, &config.server.queues.dirpath, &mail_context)
                .with_context(|| {
                    format!("cannot move file from `{}` to `{}`", queue, Queue::Dead)
                })?;

            mail_message
                .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
                .map_err(MailHandlerError::WriteMessageBody)?;
        }
        SenderOutcome::MoveToDeferred => {
            queue
                .move_to(
                    &Queue::Deferred,
                    &config.server.queues.dirpath,
                    &mail_context,
                )
                .with_context(|| {
                    format!("cannot move file from `{}` to `{}`", queue, Queue::Deferred)
                })?;

            mail_message
                .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
                .map_err(MailHandlerError::WriteMessageBody)?;
        }
        SenderOutcome::RemoveFromDisk => {
            queue.remove(&config.server.queues.dirpath, &process_message.message_id)?;
            Queue::remove_mail(&config.server.queues.dirpath, &process_message.message_id)?;
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
        mail_context::{ConnectionContext, MailContext, MessageMetadata},
        rcpt::Rcpt,
        re::tokio,
        transfer::{EmailTransferStatus, Transfer},
        MessageBody,
    };
    use vsmtp_config::build_resolvers;
    use vsmtp_rule_engine::RuleEngine;
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

        MessageBody::try_from(concat!(
            "Date: bar\r\n",
            "From: foo\r\n",
            "\r\n",
            "Hello world\r\n"
        ))
        .unwrap()
        .write_to_mails(
            &config.server.queues.dirpath,
            "message_from_deliver_to_deferred",
        )
        .unwrap();

        let rule_engine = std::sync::Arc::new(RuleEngine::from_script(&config, "#{}").unwrap());

        let resolvers = std::sync::Arc::new(build_resolvers(&config).unwrap());

        handle_one_in_delivery_queue(
            std::sync::Arc::new(config.clone()),
            resolvers,
            ProcessMessage {
                message_id: "message_from_deliver_to_deferred".to_string(),
                delegated: false,
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
