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
use crate::{log_channels, ProcessMessage};
use anyhow::Context;
use vsmtp_common::{
    mail_context::MailContext,
    queue::Queue,
    queue_path,
    re::{anyhow, log},
    state::StateSMTP,
    status::Status,
};
use vsmtp_config::{Config, Resolvers};
use vsmtp_mail_parser::MailMimeParser;
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

/// process that treats incoming email offline with the postq stage.
///
/// # Errors
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    loop {
        if let Some(pm) = working_receiver.recv().await {
            if let Err(err) = tokio::spawn(handle_one_in_working_queue(
                config.clone(),
                rule_engine.clone(),
                resolvers.clone(),
                pm,
                delivery_sender.clone(),
            ))
            .await
            {
                log::error!(target: log_channels::POSTQ, "{}", err);
            }
        }
    }
}

///
/// # Errors
///
/// # Panics
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

    let file_to_process = queue_path!(
        &config.server.queues.dirpath,
        Queue::Working,
        &process_message.message_id
    );

    log::debug!(
        target: log_channels::POSTQ,
        "(msg={}) opening file: {:?}",
        process_message.message_id,
        file_to_process
    );

    let mut ctx = MailContext::from_file(&file_to_process).context(format!(
        "failed to deserialize email in working queue '{}'",
        file_to_process.display()
    ))?;

    ctx.body = ctx.body.to_parsed::<MailMimeParser>()?;

    // locking the engine and freeing the lock before any await.
    let (state, result) = {
        let rule_engine = rule_engine
            .read()
            .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?;

        let mut state = RuleState::with_context(config.as_ref(), resolvers, &rule_engine, ctx);
        let result = rule_engine.run_when(&mut state, &StateSMTP::PostQ);

        (state, result)
    };

    if let Status::Deny(_) = result {
        Queue::Dead.write_to_queue(
            &config.server.queues.dirpath,
            &state.context().read().unwrap(),
        )?;
    } else {
        // using a bool to prevent the lock guard to reach the await call below.
        let delivered = {
            let ctx = state.context();
            let ctx = ctx.read().unwrap();

            if ctx
                .envelop
                .rcpt
                .iter()
                .all(|rcpt| rcpt.transfer_method == vsmtp_common::transfer::Transfer::None)
            {
                // skipping mime & delivery processes.
                log::warn!(
                target: log_channels::POSTQ,
                "(msg={}) delivery skipped because all recipient's transfer method is set to None.",
                process_message.message_id,
            );
                Queue::Dead.write_to_queue(&config.server.queues.dirpath, &ctx)?;
                false
            } else {
                Queue::Deliver
                    .write_to_queue(&config.server.queues.dirpath, &ctx)
                    .context(format!(
                        "failed to move '{}' from delivery queue to deferred queue",
                        process_message.message_id
                    ))?;
                true
            }
        };

        if delivered {
            delivery_sender
                .send(ProcessMessage {
                    message_id: process_message.message_id.to_string(),
                })
                .await?;
        }
    }

    std::fs::remove_file(&file_to_process).context(format!(
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
        mail_context::{Body, ConnectionContext, MailContext, MessageMetadata},
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
                    body: Body::Raw("Date: bar\r\nFrom: foo\r\nHello world\r\n".to_string()),
                    metadata: Some(MessageMetadata {
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test".to_string(),
                        skipped: None,
                    }),
                },
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
                    body: Body::Raw("Date: bar\r\nFrom: foo\r\nHello world\r\n".to_string()),
                    metadata: Some(MessageMetadata {
                        timestamp: std::time::SystemTime::now(),
                        message_id: "test_denied".to_string(),
                        skipped: None,
                    }),
                },
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
