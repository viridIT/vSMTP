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
    context_from_file_path,
    delivery::{add_trace_information, move_to_queue, send_email},
    log_channels, message_from_file_path,
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
use vsmtp_config::{Config, Resolvers};
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

pub async fn flush_deliver_queue(
    config: &Config,
    resolvers: &std::sync::Arc<Resolvers>,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let dir_entries =
        std::fs::read_dir(queue_path!(&config.server.queues.dirpath, Queue::Deliver))?;
    for path in dir_entries {
        if let Err(e) =
            handle_one_in_delivery_queue(config, resolvers, &path?.path(), rule_engine).await
        {
            log::warn!(target: log_channels::DELIVERY, "{}", e);
        }
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
    config: &Config,
    resolvers: &std::sync::Arc<Resolvers>,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::trace!(
        target: log_channels::DELIVERY,
        "email received '{message_id}'"
    );

    let ctx = context_from_file_path(path).await.with_context(|| {
        format!("failed to deserialize email in delivery queue '{message_id}'",)
    })?;

    let message_path = {
        let mut message_path = config.server.queues.dirpath.clone();
        message_path.push(format!("mails/{}", message_id));
        message_path
    };
    let message = message_from_file_path(message_path).await?;

    let (state, result) = {
        let rule_engine = rule_engine
            .read()
            .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?;

        let mut state =
            RuleState::with_context(config, resolvers.clone(), &rule_engine, ctx, Some(message));
        let result = rule_engine.run_when(&mut state, &StateSMTP::Delivery);

        (state, result)
    };

    {
        // FIXME: cloning here to prevent send_email async error with mutex guard.
        //        the context is wrapped in an RwLock because of the receiver.
        //        find a way to mutate the context in the rule engine without
        //        using a RwLock.
        let mut ctx = state.context().read().unwrap().clone();
        let mut message = state.message().read().unwrap().as_ref().unwrap().clone();

        add_trace_information(config, &mut ctx, &mut message, &result)?;

        if let Status::Deny(_) = result {
            // we update rcpt email status and write to dead queue in case of a deny.
            for rcpt in &mut ctx.envelop.rcpt {
                rcpt.email_status =
                    EmailTransferStatus::Failed("rule engine denied the email.".to_string());
            }
            Queue::Dead.write_to_queue(&config.server.queues.dirpath, &ctx)?;
        } else {
            let metadata = ctx
                .metadata
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("metadata not available on delivery"))?;

            ctx.envelop.rcpt = send_email(
                config,
                resolvers,
                metadata,
                &ctx.envelop.mail_from,
                &ctx.envelop.rcpt,
                &message,
            )
            .await
            .context(format!(
                "failed to send '{message_id}' located in the delivery queue"
            ))?;

            move_to_queue(config, &ctx)?;
        }
    }

    // after processing the email is removed from the delivery queue.
    std::fs::remove_file(path).context(format!(
        "failed to remove '{message_id}' from the delivery queue"
    ))?;

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
            &MessageBody::Raw(
                ["Date: bar", "From: foo", "Hello world"]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            ),
        )
        .unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::from_script(&config, "#{}").unwrap(),
        ));

        let resolvers = std::sync::Arc::new(build_resolvers(&config).unwrap());

        handle_one_in_delivery_queue(
            &config,
            &resolvers,
            &queue_path!(
                &config.server.queues.dirpath,
                Queue::Deliver,
                "message_from_deliver_to_deferred"
            ),
            &rule_engine,
        )
        .await
        .unwrap();

        std::fs::remove_file(queue_path!(
            &config.server.queues.dirpath,
            Queue::Deferred,
            "message_from_deliver_to_deferred"
        ))
        .unwrap();
    }
}
