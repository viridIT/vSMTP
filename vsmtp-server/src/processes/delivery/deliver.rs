use crate::processes::delivery::{add_trace_information, move_to_queue, send_email};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MailContext,
    queue::Queue,
    re::{
        anyhow::{self, Context},
        log,
    },
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{log_channel::DELIVER, Config};
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

pub async fn flush_deliver_queue(
    config: &Config,
    dns: &TokioAsyncResolver,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let dir_entries = std::fs::read_dir(Queue::Deliver.to_path(&config.server.queues.dirpath)?)?;
    for path in dir_entries {
        if let Err(e) = handle_one_in_delivery_queue(config, dns, &path?.path(), rule_engine).await
        {
            log::warn!("{}", e);
        }
    }

    Ok(())
}

/// handle and send one email pulled from the delivery queue.
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
    dns: &TokioAsyncResolver,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::trace!(
        target: DELIVER,
        "vDeliver (delivery) email received '{}'",
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

        add_trace_information(config, &mut ctx, result)?;

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
                dns,
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
        mail_context::{Body, MailContext, MessageMetadata},
        rcpt::Rcpt,
        transfer::{EmailTransferStatus, Transfer},
    };
    use vsmtp_config::build_dns;
    use vsmtp_rule_engine::rule_engine::RuleEngine;
    use vsmtp_test::config;

    #[tokio::test]
    async fn basic() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();

        let now = std::time::SystemTime::now();

        let dns = build_dns(&config).unwrap();

        Queue::Deliver
            .write_to_queue(
                &config.server.queues.dirpath,
                &MailContext {
                    connection_timestamp: now,
                    client_addr: "127.0.0.1:80".parse().unwrap(),
                    envelop: Envelop {
                        helo: "client.com".to_string(),
                        mail_from: Address::try_from("from@client.com".to_string()).unwrap(),
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
            RuleEngine::from_script("#{}").unwrap(),
        ));

        handle_one_in_delivery_queue(
            &config,
            &dns,
            &Queue::Deliver
                .to_path(&config.server.queues.dirpath)
                .unwrap()
                .join("message_from_deliver_to_deferred"),
            &rule_engine,
        )
        .await
        .unwrap();

        std::fs::remove_file(
            Queue::Deferred
                .to_path(&config.server.queues.dirpath)
                .unwrap()
                .join("message_from_deliver_to_deferred"),
        )
        .unwrap();
    }
}
