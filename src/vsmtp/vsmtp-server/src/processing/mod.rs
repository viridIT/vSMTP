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
use crate::{delegate, log_channels, receiver::MailHandlerError, Process, ProcessMessage};
use anyhow::Context;
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    queue::Queue,
    queue_path,
    re::{anyhow, log, tokio},
    state::StateSMTP,
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{create_app_folder, Config, Resolvers};
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) {
    loop {
        if let Some(pm) = working_receiver.recv().await {
            tokio::spawn(handle_one_in_working_queue(
                config.clone(),
                rule_engine.clone(),
                resolvers.clone(),
                pm,
                delivery_sender.clone(),
            ));
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_one_in_working_queue(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) {
    log::info!(
        target: log_channels::POSTQ,
        "handling message in working queue {}",
        process_message.message_id
    );

    if let Err(e) = handle_one_in_working_queue_inner(
        config,
        rule_engine,
        resolvers,
        process_message,
        delivery_sender,
    )
    .await
    {
        log::warn!(
            target: log_channels::POSTQ,
            "failed to handle one email in working queue: {}",
            e
        );
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_one_in_working_queue_inner(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    log::debug!(
        target: log_channels::POSTQ,
        "received a new message: {}",
        process_message.message_id,
    );

    let (context_working_filepath, message_working_filepath) = (
        queue_path!(
            &config.server.queues.dirpath,
            Queue::Working,
            &process_message.message_id
        ),
        std::path::PathBuf::from_iter([
            config.server.queues.dirpath.clone(),
            "mails".into(),
            process_message.message_id.clone().into(),
        ]),
    );

    log::debug!(
        target: log_channels::POSTQ,
        "(msg={}) opening file: ctx=`{}` msg=`{}`",
        process_message.message_id,
        context_working_filepath.display(),
        message_working_filepath.display(),
    );

    let (context_working, message_working) = tokio::join!(
        MailContext::from_file_path(&context_working_filepath),
        MessageBody::from_file_path(message_working_filepath)
    );

    let (context_working, message_working) = (
        context_working.with_context(|| {
            format!(
                "failed to deserialize email in working queue '{}'",
                context_working_filepath.display()
            )
        })?,
        message_working.context("error while reading message")?,
    );

    let (mut context, message, _, skipped) = RuleState::just_run_when(
        &StateSMTP::PostQ,
        config.as_ref(),
        resolvers,
        &rule_engine,
        context_working,
        message_working,
    )?;

    let mut write_to_queue = Option::<Queue>::None;
    let mut send_to_delivery = false;
    let mut write_email = true;

    match &skipped {
        Some(Status::Quarantine(path)) => {
            let mut path = create_app_folder(&config, Some(path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &context)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            std::fs::remove_file(&context_working_filepath).context(format!(
                "failed to remove '{}' from the working queue",
                process_message.message_id
            ))?;

            log::warn!(
                target: log_channels::POSTQ,
                "[{}/postq] skipped due to quarantine.",
                context.connection.server_address
            );
        }
        Some(Status::Delegated(delegator)) => {
            context.metadata.as_mut().unwrap().skipped = None;

            // FIXME: find a way to use `write_to_queue` instead to be consistant
            //        with the rest of the function.
            Queue::Working
                .write_to_queue(&config.server.queues.dirpath, &context)
                .map_err(|error| MailHandlerError::WriteToQueue(Queue::Working, error))?;

            Queue::write_to_mails(
                &config.server.queues.dirpath,
                &process_message.message_id,
                &message,
            )
            .map_err(MailHandlerError::WriteMessageBody)?;

            // NOTE: needs to be executed after writing, because the other
            //       thread could pickup the email faster than this function.
            delegate(delegator, &context, &message).map_err(MailHandlerError::DelegateMessage)?;

            write_email = false;

            log::warn!(
                target: log_channels::POSTQ,
                "[{}/postq] skipped due to delegation.",
                context.connection.server_address
            );
        }
        Some(Status::DelegationResult) => {
            send_to_delivery = true;
            write_to_queue = Some(Queue::Deliver);
        }
        Some(Status::Deny(code)) => {
            for rcpt in &mut context.envelop.rcpt {
                rcpt.email_status = EmailTransferStatus::Failed(format!(
                    "rule engine denied the email in delivery: {code:?}."
                ));
            }

            write_to_queue = Some(Queue::Dead);
        }
        Some(reason) => {
            log::warn!(
                target: log_channels::POSTQ,
                "[{}/postq] skipped due to '{}'.",
                context.connection.server_address,
                reason.as_ref()
            );
            write_to_queue = Some(Queue::Deliver);
            send_to_delivery = true;
        }
        None => {
            write_to_queue = Some(Queue::Deliver);
            send_to_delivery = true;
        }
    };

    // FIXME: sending the email down a ProcessMessage instead
    //        of writing on disk would be great here.
    if write_email {
        Queue::write_to_mails(
            &config.server.queues.dirpath,
            &process_message.message_id,
            &message,
        )
        .map_err(MailHandlerError::WriteMessageBody)?;

        log::debug!(
            target: log_channels::TRANSACTION,
            "[{}/postq] (msg={}) email written in 'mails' queue.",
            context.connection.server_address,
            process_message.message_id
        );
    }

    if let Some(queue) = write_to_queue {
        queue
            .write_to_queue(&config.server.queues.dirpath, &context)
            .map_err(|error| MailHandlerError::WriteToQueue(queue, error))?;

        // we remove the old working queue message only
        // if the message as not been overwritten already.
        if !matches!(queue, Queue::Working) {
            std::fs::remove_file(&context_working_filepath).context(format!(
                "failed to remove '{}' from the working queue",
                process_message.message_id
            ))?;
        }
    }

    if send_to_delivery {
        delivery_sender
            .send(ProcessMessage {
                message_id: process_message.message_id.clone(),
            })
            .await
            .map_err(|error| MailHandlerError::SendToNextProcess(Process::Delivery, error))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProcessMessage;
    use vsmtp_common::{
        addr,
        envelop::Envelop,
        mail_context::{ConnectionContext, MailContext, MessageBody, MessageMetadata},
        rcpt::Rcpt,
        re::anyhow::Context,
        transfer::{EmailTransferStatus, Transfer},
    };
    use vsmtp_rule_engine::rule_engine::RuleEngine;
    use vsmtp_test::config;

    #[tokio::test]
    async fn cannot_deserialize() {
        let config = config::local_test();

        let (delivery_sender, _delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        assert!(handle_one_in_working_queue_inner(
            config.clone(),
            std::sync::Arc::new(std::sync::RwLock::new(
                RuleEngine::from_script(&config, "#{}")
                    .context("failed to initialize the engine")
                    .unwrap(),
            )),
            resolvers,
            ProcessMessage {
                message_id: "not_such_message_named_like_this".to_string(),
            },
            delivery_sender,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn basic() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();

        Queue::Working
            .write_to_queue(
                &config.server.queues.dirpath,
                &MailContext {
                    connection: ConnectionContext {
                        timestamp: std::time::SystemTime::now(),
                        credentials: None,
                        is_authenticated: false,
                        is_secured: false,
                        server_name: "testserver.com".to_string(),
                        server_address: "127.0.0.1:25".parse().unwrap(),
                    },
                    client_addr: "127.0.0.1:80".parse().unwrap(),
                    envelop: Envelop {
                        helo: "client.com".to_string(),
                        mail_from: addr!("from@client.com"),
                        rcpt: vec![
                            Rcpt {
                                address: addr!("to+1@client.com"),
                                transfer_method: Transfer::Deliver,
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
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test".to_string(),
                        skipped: None,
                    }),
                },
            )
            .unwrap();

        Queue::write_to_mails(
            &config.server.queues.dirpath,
            "test",
            &MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string()],
                body: Some("Hello world".to_string()),
            },
        )
        .unwrap();

        let (delivery_sender, mut delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue_inner(
            config.clone(),
            std::sync::Arc::new(std::sync::RwLock::new(
                RuleEngine::from_script(&config, "#{}")
                    .context("failed to initialize the engine")
                    .unwrap(),
            )),
            resolvers,
            ProcessMessage {
                message_id: "test".to_string(),
            },
            delivery_sender,
        )
        .await
        .unwrap();

        assert_eq!(delivery_receiver.recv().await.unwrap().message_id, "test");
        assert!(!std::path::PathBuf::from("./tmp/working/test").exists());
        assert!(std::path::PathBuf::from("./tmp/deliver/test").exists());
    }

    #[tokio::test]
    async fn denied() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();

        Queue::Working
            .write_to_queue(
                &config.server.queues.dirpath,
                &MailContext {
                    connection: ConnectionContext {
                        timestamp: std::time::SystemTime::now(),
                        credentials: None,
                        is_authenticated: false,
                        is_secured: false,
                        server_name: "testserver.com".to_string(),
                        server_address: "127.0.0.1:25".parse().unwrap(),
                    },
                    client_addr: "127.0.0.1:80".parse().unwrap(),
                    envelop: Envelop {
                        helo: "client.com".to_string(),
                        mail_from: addr!("from@client.com"),
                        rcpt: vec![
                            Rcpt {
                                address: addr!("to+1@client.com"),
                                transfer_method: Transfer::Deliver,
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
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test_denied".to_string(),
                        skipped: None,
                    }),
                },
            )
            .unwrap();

        Queue::write_to_mails(
            &config.server.queues.dirpath,
            "test_denied",
            &MessageBody::Raw {
                headers: vec!["Date: bar".to_string(), "From: foo".to_string()],
                body: Some("Hello world".to_string()),
            },
        )
        .unwrap();

        let (delivery_sender, _delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue_inner(
            config.clone(),
            std::sync::Arc::new(std::sync::RwLock::new(
                RuleEngine::from_script(
                    &config,
                    &format!("#{{ {}: [ rule \"\" || sys::deny() ] }}", StateSMTP::PostQ),
                )
                .context("failed to initialize the engine")
                .unwrap(),
            )),
            resolvers,
            ProcessMessage {
                message_id: "test_denied".to_string(),
            },
            delivery_sender,
        )
        .await
        .unwrap();

        assert!(!std::path::PathBuf::from("./tmp/working/test_denied").exists());
        assert!(std::path::PathBuf::from("./tmp/dead/test_denied").exists());
    }
}
