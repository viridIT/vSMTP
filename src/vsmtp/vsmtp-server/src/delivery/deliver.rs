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
    delivery::{add_trace_information, send_mail},
    log_channels,
    receiver::MailHandlerError,
    ProcessMessage,
};
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
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

#[allow(clippy::too_many_lines)]
async fn handle_one_in_delivery_queue_inner(
    config: std::sync::Arc<Config>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let (context_deliver_filepath, mut message_deliver_filepath) = (
        queue_path!(
            &config.server.queues.dirpath,
            if process_message.delegated {
                Queue::Delegated
            } else {
                Queue::Deliver
            },
            &process_message.message_id
        ),
        std::path::PathBuf::from_iter([
            config.server.queues.dirpath.clone(),
            "mails".into(),
            process_message.message_id.clone().into(),
        ]),
    );

    let context_delivery = MailContext::from_file_path(&context_deliver_filepath)
        .await
        .with_context(|| {
            format!(
                "failed to deserialize email in delivery queue '{}'",
                process_message.message_id
            )
        })?;

    let message_delivery = MessageBody::from_file_path(message_deliver_filepath.clone()).await?;

    let (mut mail_context, mut mail_message, result, skipped) = RuleState::just_run_when(
        &StateSMTP::Delivery,
        config.as_ref(),
        resolvers.clone(),
        &rule_engine,
        context_delivery,
        message_delivery,
    )?;

    let mut write_to_queue = Option::<Queue>::None;

    match &skipped {
        Some(Status::Quarantine(path)) => {
            let mut path = create_app_folder(&config, Some(path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &mail_context)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            std::fs::remove_file(&context_deliver_filepath).context(format!(
                "failed to remove '{}' from the working queue",
                process_message.message_id
            ))?;

            log::warn!(
                target: log_channels::DELIVERY,
                "[{}/delivery] skipped due to quarantine.",
                mail_context.connection.server_address
            );

            return Ok(());
        }
        Some(Status::Delegated(delegator)) => {
            mail_context.metadata.as_mut().unwrap().skipped = Some(Status::DelegationResult);

            // FIXME: find a way to use `write_to_queue` instead to be consistant
            //        with the rest of the function.
            Queue::Delegated
                .write_to_queue(&config.server.queues.dirpath, &mail_context)
                .map_err(|error| MailHandlerError::WriteToQueue(Queue::Working, error))?;

            // NOTE: needs to be executed after writing, because the other
            //       thread could pickup the email faster than this function.
            delegate(delegator, &mail_context, &mail_message)
                .map_err(MailHandlerError::DelegateMessage)?;

            log::warn!(
                target: log_channels::DELIVERY,
                "[{}/delivery] skipped due to delegation.",
                mail_context.connection.server_address
            );
        }
        Some(Status::DelegationResult) => unreachable!(
            "delivery is the last stage, delegation results cannot travel down any further."
        ),
        Some(Status::Deny(code)) => {
            for rcpt in &mut mail_context.envelop.rcpt {
                rcpt.email_status = EmailTransferStatus::Failed(format!(
                    "rule engine denied the email in delivery: {code:?}."
                ));
            }

            write_to_queue = Some(Queue::Dead);
        }
        Some(reason) => {
            log::warn!(
                target: log_channels::DELIVERY,
                "[{}/delivery] skipped due to '{}'.",
                mail_context.connection.server_address,
                reason.as_ref()
            );
        }
        None => {}
    };

    add_trace_information(&config, &mut mail_context, &mut mail_message, &result)?;

    if let Some(queue) = write_to_queue {
        // writing the whole email anyway because it
        // has not being sent.
        Queue::write_to_mails(
            &config.server.queues.dirpath,
            &process_message.message_id,
            &mail_message,
        )
        .map_err(MailHandlerError::WriteMessageBody)?;

        log::debug!(
            target: log_channels::DELIVERY,
            "[{}/delivery] (msg={}) email written in 'mails' queue.",
            mail_context.connection.server_address,
            process_message.message_id
        );

        queue
            .write_to_queue(&config.server.queues.dirpath, &mail_context)
            .map_err(|error| MailHandlerError::WriteToQueue(queue, error))?;

        // we remove the old working queue message only
        // if the message as not been overwritten already.
        if !matches!(queue, Queue::Working) {
            std::fs::remove_file(&context_deliver_filepath).context(format!(
                "failed to remove '{}' from the deliver queue",
                process_message.message_id
            ))?;
        }
    } else {
        send_mail(&config, &mut mail_context, &mail_message, &resolvers).await;

        let success = if mail_context
            .envelop
            .rcpt
            .iter()
            .any(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
        {
            Queue::Deferred
                .write_to_queue(&config.server.queues.dirpath, &mail_context)
                .context("failed to move message from delivery queue to deferred queue")?;

            false
        } else {
            true
        };

        let success = if mail_context
            .envelop
            .rcpt
            .iter()
            .any(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::Failed(..)))
        {
            Queue::Dead
                .write_to_queue(&config.server.queues.dirpath, &mail_context)
                .context("failed to move message from delivery queue to dead queue")?;

            false
        } else {
            success
        };

        std::fs::remove_file(&context_deliver_filepath).context(format!(
            "failed to remove '{}' from the delivery queue",
            process_message.message_id
        ))?;

        if success {
            message_deliver_filepath.set_extension(match mail_message {
                MessageBody::Raw { .. } => "eml",
                MessageBody::Parsed(_) => "json",
            });

            std::fs::remove_file(&message_deliver_filepath).context(format!(
                "failed to remove {:?} from the mail queue",
                message_deliver_filepath
            ))?;
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
