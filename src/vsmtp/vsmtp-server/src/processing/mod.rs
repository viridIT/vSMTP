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
use crate::{delegate, receiver::MailHandlerError, Process, ProcessMessage};
use vsmtp_common::{
    queue::Queue,
    re::{anyhow, log, tokio},
    state::StateSMTP,
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{create_app_folder, Config, Resolvers};
use vsmtp_rule_engine::{RuleEngine, RuleState};

pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<RuleEngine>,
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

#[tracing::instrument(skip(config, rule_engine, resolvers, delivery_sender))]
async fn handle_one_in_working_queue(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<RuleEngine>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) {
    log::info!("handling message in `{queue}`", queue = Queue::Working);

    if let Err(e) = handle_one_in_working_queue_inner(
        config,
        rule_engine,
        resolvers,
        process_message,
        delivery_sender,
    )
    .await
    {
        log::warn!("failed to handle one email in working queue: `{e}`");
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_one_in_working_queue_inner(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<RuleEngine>,
    resolvers: std::sync::Arc<Resolvers>,
    process_message: ProcessMessage,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    let queue = if process_message.delegated {
        Queue::Delegated
    } else {
        Queue::Working
    };

    let (mail_context, mail_message) = queue
        .read(&config.server.queues.dirpath, &process_message.message_id)
        .await?;

    let (mut mail_context, mail_message, _, skipped) = RuleState::just_run_when(
        &StateSMTP::PostQ,
        config.as_ref(),
        resolvers,
        &rule_engine,
        mail_context,
        mail_message,
    );

    let mut move_to_queue = Option::<Queue>::None;
    let mut send_to_delivery = false;
    let mut write_email = true;
    let mut delegated = false;

    match &skipped {
        Some(Status::Quarantine(path)) => {
            let mut path = create_app_folder(&config, Some(path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &mail_context)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            queue.remove(&config.server.queues.dirpath, &process_message.message_id)?;

            log::warn!("skipped due to quarantine.");
        }
        Some(Status::Delegated(delegator)) => {
            mail_context.metadata.as_mut().unwrap().skipped = Some(Status::DelegationResult);

            // FIXME: find a way to use `write_to_queue` instead to be consistent
            //        with the rest of the function.
            // NOTE:  moving here because the delegation process could try to
            //        pickup the email before it's written on disk.
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

            write_email = false;
            delegated = true;

            log::warn!("skipped due to delegation.");
        }
        Some(Status::DelegationResult) => {
            send_to_delivery = true;
            delegated = true;
        }
        Some(Status::Deny(code)) => {
            for rcpt in &mut mail_context.envelop.rcpt {
                rcpt.email_status = EmailTransferStatus::Failed {
                    timestamp: std::time::SystemTime::now(),
                    reason: format!("rule engine denied the message in postq: {code:?}."),
                };
            }

            move_to_queue = Some(Queue::Dead);
        }
        Some(reason) => {
            log::warn!("skipped due to '{}'.", reason.as_ref());
            move_to_queue = Some(Queue::Deliver);
            send_to_delivery = true;
        }
        None => {
            move_to_queue = Some(Queue::Deliver);
            send_to_delivery = true;
        }
    };

    // FIXME: sending the email down a ProcessMessage instead
    //        of writing on disk would be great here.
    if write_email {
        mail_message
            .write_to_mails(&config.server.queues.dirpath, &process_message.message_id)
            .map_err(MailHandlerError::WriteMessageBody)?;

        log::debug!("email written in 'mails' queue.");
    }

    if let Some(next_queue) = move_to_queue {
        queue.move_to(&next_queue, &config.server.queues.dirpath, &mail_context)?;
    }

    if send_to_delivery {
        delivery_sender
            .send(ProcessMessage {
                message_id: process_message.message_id.clone(),
                delegated,
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
        mail_context::{ConnectionContext, MailContext, MessageMetadata},
        rcpt::Rcpt,
        transfer::{EmailTransferStatus, Transfer},
        MessageBody,
    };
    use vsmtp_rule_engine::RuleEngine;
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
            std::sync::Arc::new(RuleEngine::from_script(&config, "#{}").unwrap()),
            resolvers,
            ProcessMessage {
                message_id: "not_such_message_named_like_this".to_string(),
                delegated: false,
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
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test".to_string(),
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
        .write_to_mails(&config.server.queues.dirpath, "test")
        .unwrap();

        let (delivery_sender, mut delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue_inner(
            config.clone(),
            std::sync::Arc::new(RuleEngine::from_script(&config, "#{}").unwrap()),
            resolvers,
            ProcessMessage {
                message_id: "test".to_string(),
                delegated: false,
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
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test_denied".to_string(),
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
        .write_to_mails(&config.server.queues.dirpath, "test_denied")
        .unwrap();

        let (delivery_sender, _delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue_inner(
            config.clone(),
            std::sync::Arc::new(
                RuleEngine::from_script(
                    &config,
                    &format!("#{{ {}: [ rule \"\" || sys::deny() ] }}", StateSMTP::PostQ),
                )
                .unwrap(),
            ),
            resolvers,
            ProcessMessage {
                message_id: "test_denied".to_string(),
                delegated: false,
            },
            delivery_sender,
        )
        .await
        .unwrap();

        assert!(!std::path::PathBuf::from("./tmp/working/test_denied").exists());
        assert!(std::path::PathBuf::from("./tmp/dead/test_denied").exists());
    }
}
