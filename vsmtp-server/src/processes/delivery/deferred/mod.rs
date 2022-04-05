use crate::{processes::delivery::send_email, queue::Queue};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        log,
    },
    transfer::EmailTransferStatus,
};
use vsmtp_config::{log_channel::DELIVER, Config};

pub async fn flush_deferred_queue(config: &Config, dns: &TokioAsyncResolver) -> anyhow::Result<()> {
    let dir_entries = std::fs::read_dir(Queue::Deferred.to_path(&config.server.queues.dirpath)?)?;
    for path in dir_entries {
        if let Err(e) = handle_one_in_deferred_queue(config, dns, &path?.path()).await {
            log::warn!("{}", e);
        }
    }

    Ok(())
}

// NOTE: emails stored in the deferred queue are likely to slow down the process.
//       the pickup process of this queue should be slower than pulling from the delivery queue.
//       https://www.postfix.org/QSHAPE_README.html#queues
async fn handle_one_in_deferred_queue(
    config: &Config,
    dns: &TokioAsyncResolver,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::debug!(
        target: DELIVER,
        "vDeliver (deferred) processing email '{}'",
        message_id
    );

    let mut ctx = MailContext::from_file(path).with_context(|| {
        format!(
            "failed to deserialize email in deferred queue '{}'",
            &message_id
        )
    })?;

    let max_retry_deferred = config.server.queues.delivery.deferred_retry_max;

    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("email metadata not available in deferred email"))?;

    // TODO: at this point, only HeldBack recipients should be present in the queue.
    //       check if it is true or not.
    ctx.envelop.rcpt = send_email(
        config,
        dns,
        metadata,
        &ctx.envelop.mail_from,
        &ctx.envelop.rcpt,
        &ctx.body,
    )
    .await
    .context("failed to send emails from the deferred queue")?;

    // updating retry count, set status to Failed if threshold reached.
    ctx.envelop.rcpt = ctx
        .envelop
        .rcpt
        .into_iter()
        .map(|mut rcpt| {
            rcpt.email_status = match rcpt.email_status {
                EmailTransferStatus::HeldBack(count) if count >= max_retry_deferred => {
                    EmailTransferStatus::Failed(format!(
                        "maximum retry count of '{max_retry_deferred}' reached"
                    ))
                }
                EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count + 1),
                status => EmailTransferStatus::Failed(format!(
                    "wrong recipient status '{status}' found in the deferred queue"
                )),
            };
            rcpt
        })
        .collect();

    // if there are no recipients left to send the email to, we remove the file from the deferred queue.
    if ctx
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| !matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
    {
        std::fs::remove_file(&path)?;
    } else {
        // otherwise, we just update the recipient list on disk.
        Queue::Deferred.write_to_queue(config, &ctx)?;
    }

    Ok(())
}
