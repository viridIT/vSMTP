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
    context_from_file_path, log_channels, message_from_file_path, receiver::MailHandlerError,
    ProcessMessage,
};
use anyhow::Context;
use vsmtp_common::{
    queue::Queue,
    queue_path,
    re::{anyhow, log, tokio},
    state::StateSMTP,
    status::Status,
    transfer::Transfer,
};
use vsmtp_config::{create_app_folder, Config, Resolvers};
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
    log::debug!(
        target: log_channels::POSTQ,
        "received a new message: {}",
        process_message.message_id,
    );

    let (context_filepath, message_filepath) = (
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
        context_filepath.display(),
        message_filepath.display(),
    );

    let (ctx, message) = tokio::join!(
        context_from_file_path(&context_filepath),
        message_from_file_path(message_filepath)
    );
    let (ctx, message) = (
        ctx.with_context(|| {
            format!(
                "failed to deserialize email in working queue '{}'",
                context_filepath.display()
            )
        })?,
        message.context("error while reading message")?,
    );

    let ((ctx, message), result) = {
        let rule_engine = rule_engine
            .read()
            .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?;

        let mut state =
            RuleState::with_context(config.as_ref(), resolvers, &rule_engine, ctx, Some(message));
        let result = rule_engine.run_when(&mut state, &StateSMTP::PostQ);

        (state.take()?, result)
    };

    // writing the mails in any case because we don't know (yet) if it changed
    Queue::write_to_mails(
        &config.server.queues.dirpath,
        &process_message.message_id,
        &message.ok_or_else(|| anyhow::anyhow!("message is empty"))?,
    )?;

    let queue = match result {
        Status::Quarantine(path) => {
            let mut path = create_app_folder(&config, Some(&path))
                .map_err(MailHandlerError::CreateAppFolder)?;

            path.push(format!("{}.json", process_message.message_id));

            Queue::write_to_quarantine(&path, &ctx)
                .await
                .map_err(MailHandlerError::WriteQuarantineFile)?;

            std::fs::remove_file(&context_filepath).context(format!(
                "failed to remove '{}' from the working queue",
                process_message.message_id
            ))?;

            log::warn!("delivery skipped due to quarantine.");
            return Ok(());
        }
        Status::Deny(_) => Queue::Dead,
        _ if ctx
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::None) =>
        {
            log::warn!(
                target: log_channels::POSTQ,
                "(msg={}) delivery skipped because all recipient's transfer method is set to None.",
                process_message.message_id,
            );
            Queue::Dead
        }
        _ => Queue::Deliver,
    };

    queue
        .write_to_queue(&config.server.queues.dirpath, &ctx)
        .context(format!(
            "failed to move '{}' from delivery queue to deferred queue",
            process_message.message_id
        ))?;

    if queue != Queue::Dead {
        delivery_sender
            .send(ProcessMessage {
                message_id: process_message.message_id.to_string(),
            })
            .await?;
    }

    std::fs::remove_file(&context_filepath).context(format!(
        "failed to remove '{}' from the working queue",
        process_message.message_id
    ))?;

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

        assert!(handle_one_in_working_queue(
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
            &MessageBody::Raw(
                ["Date: bar", "From: foo", "Hello world"]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            ),
        )
        .unwrap();

        let (delivery_sender, mut delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue(
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
            &MessageBody::Raw(
                ["Date: bar", "From: foo", "Hello world"]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            ),
        )
        .unwrap();

        let (delivery_sender, _delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let config = std::sync::Arc::new(config);

        let resolvers = std::sync::Arc::new(
            vsmtp_config::build_resolvers(&config).expect("could not initialize dns"),
        );

        handle_one_in_working_queue(
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
