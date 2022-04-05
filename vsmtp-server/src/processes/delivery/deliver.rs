use crate::{
    processes::delivery::{add_trace_information, move_to_queue, send_email},
    queue::Queue,
};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MailContext,
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
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.server.queues.dirpath)?)? {
        let path = path?;
        let message_id = path.file_name();

        if let Err(e) = handle_one_in_delivery_queue(
            config,
            dns,
            message_id
                .to_str()
                .context("could not fetch message id in delivery queue")?,
            &path.path(),
            rule_engine,
        )
        .await
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
    message_id: &str,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    log::trace!(
        target: DELIVER,
        "vDeliver (delivery) email received '{}'",
        message_id
    );

    let ctx = MailContext::from_file(path).with_context(|| {
        format!(
            "failed to deserialize email in delivery queue '{}'",
            &message_id
        )
    })?;

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
            Queue::Dead.write_to_queue(config, &ctx)?;
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
            .with_context(|| {
                format!("failed to send '{message_id}' located in the delivery queue")
            })?;

            move_to_queue(config, &ctx)?;
        };
    }

    // after processing the email is removed from the delivery queue.
    std::fs::remove_file(path)
        .with_context(|| format!("failed to remove '{message_id}' from the delivery queue"))?;

    Ok(())
}
