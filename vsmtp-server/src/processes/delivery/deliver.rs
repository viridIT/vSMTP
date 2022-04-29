/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::{
    log_channels,
    processes::delivery::{add_trace_information, move_to_queue, send_email},
};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MailContext,
    queue::Queue,
    queue_path,
    re::{
        anyhow::{self, Context},
        log,
    },
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

/// read all entries from the deliver queue & tries to send them.
pub async fn flush_deliver_queue(
    config: &Config,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
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
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::trace!(
        target: log_channels::DELIVERY,
        "email received '{}'",
        message_id
    );

    let ctx = MailContext::from_file(path).context(format!(
        "failed to deserialize email in delivery queue '{}'",
        &message_id
    ))?;

    let mut state = RuleState::with_context(config, ctx);

    let result = rule_engine
        .read()
        .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?
        .run_when(&mut state, &vsmtp_common::state::StateSMTP::Delivery);
    {
        // FIXME: cloning here to prevent send_email async error with mutex guard.
        //        the context is wrapped in an RwLock because of the receiver.
        //        find a way to mutate the context in the rule engine without
        //        using a RwLock.
        let mut ctx = state.get_context().read().unwrap().clone();

        add_trace_information(config, &mut ctx, &result)?;

        if result == Status::Deny {
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
                &ctx.body,
            )
            .await
            .context(format!(
                "failed to send '{message_id}' located in the delivery queue"
            ))?;

            move_to_queue(config, &ctx)?;
        };
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
        address::Address,
        envelop::Envelop,
        mail_context::{Body, ConnectionContext, MailContext, MessageMetadata},
        rcpt::Rcpt,
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

        let resolvers = build_resolvers(&config).unwrap();

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
                        mail_from: Address::try_from("from@testserver.com".to_string()).unwrap(),
                        rcpt: vec![
                            Rcpt {
                                address: Address::try_from("to+1@client.com".to_string()).unwrap(),
                                transfer_method: Transfer::Maildir,
                                email_status: EmailTransferStatus::Waiting,
                            },
                            Rcpt {
                                address: Address::try_from("to+2@client.com".to_string()).unwrap(),
                                transfer_method: Transfer::Maildir,
                                email_status: EmailTransferStatus::Waiting,
                            },
                        ],
                    },
                    body: Body::Raw("Date: bar\r\nFrom: foo\r\nHello world\r\n".to_string()),
                    metadata: Some(MessageMetadata {
                        timestamp: now,
                        message_id: "message_from_deliver_to_deferred".to_string(),
                        skipped: None,
                    }),
                },
            )
            .unwrap();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::from_script(&config, "#{}").unwrap(),
        ));

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
