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
use super::ProcessMessage;
use crate::queue::Queue;
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::{Body, MailContext},
    status::Status,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{log_channel::DELIVER, Config};
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

/// process used to deliver incoming emails force accepted by the smtp process
/// or parsed by the vMime process.
///
/// # Errors
///
/// *
///
/// # Panics
///
/// * tokio::select!
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) -> anyhow::Result<()> {
    log::info!(
        target: DELIVER,
        "vDeliver (delivery) booting, flushing queue.",
    );

    let dns = vsmtp_config::trust_dns_helper::build_dns(&config)
        .context("could not initialize the delivery dns")?;

    flush_deliver_queue(&config, &dns, &rule_engine).await?;

    let mut flush_deferred_interval =
        tokio::time::interval(config.server.queues.delivery.deferred_retry_period);

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                // FIXME: transports are mutable, so must be in a mutex
                // for a delivery in a separated thread...
                if let Err(error) = handle_one_in_delivery_queue(
                    &config,
                    &dns,
                    &pm.message_id,
                    &std::path::PathBuf::from_iter([
                        Queue::Deliver.to_path(&config.server.queues.dirpath)?,
                        std::path::Path::new(&pm.message_id).to_path_buf(),
                    ]),
                    &rule_engine,
                )
                .await {
                    log::error!(target: DELIVER, "could not deliver email '{}': {error:?}", pm.message_id);
                }
            }
            _ = flush_deferred_interval.tick() => {
                log::info!(
                    target: DELIVER,
                    "vDeliver (deferred) cronjob delay elapsed, flushing queue.",
                );
                flush_deferred_queue(&config, &dns).await?;
            }
        };
    }
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

    let file = std::fs::File::open(path).context("failed to open mail in delivery queue")?;
    let reader = std::io::BufReader::new(file);
    let ctx: MailContext =
        serde_json::from_reader(reader).context("failed to read email from delivery queue")?;

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
            .context("failed to send emails from the delivery queue")?;

            move_to_queue(config, message_id, path, &ctx.envelop.rcpt)?;
        };
    }

    // after processing the email is removed from the delivery queue.
    std::fs::remove_file(path)?;

    Ok(())
}

async fn flush_deliver_queue(
    config: &Config,
    dns: &TokioAsyncResolver,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.server.queues.dirpath)?)? {
        let path = path?;
        let message_id = path.file_name();

        handle_one_in_delivery_queue(
            config,
            dns,
            message_id
                .to_str()
                .context("could not fetch message id in delivery queue")?,
            &path.path(),
            rule_engine,
        )
        .await?;
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
        "vDeliver (deferred) received email '{}'",
        message_id
    );

    let file = std::fs::File::open(path).context("failed to open mail in deferred queue")?;
    let reader = std::io::BufReader::new(file);
    let mut ctx: MailContext =
        serde_json::from_reader(reader).context("failed to read email from deferred queue")?;

    let max_retry_deferred = config.server.queues.delivery.deferred_retry_max;

    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("email metadata not available in deferred email"))?;

    // TODO: at this point, only HeldBack recipients should be present.
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
                    EmailTransferStatus::Failed("max retry reached".to_string())
                }
                EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count + 1),
                status => status,
            };
            rcpt
        })
        .collect();

    if ctx
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| !matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
    {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}

async fn flush_deferred_queue(config: &Config, dns: &TokioAsyncResolver) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deferred.to_path(&config.server.queues.dirpath)?)? {
        handle_one_in_deferred_queue(config, dns, &path?.path()).await?;
    }

    Ok(())
}

/// send the email following each recipient transport method.
/// return a list of recipients with updated email_status field.
/// recipients tagged with the Sent email_status are discarded.
async fn send_email(
    config: &Config,
    dns: &TokioAsyncResolver,
    metadata: &vsmtp_common::mail_context::MessageMetadata,
    from: &vsmtp_common::address::Address,
    to: &[vsmtp_common::rcpt::Rcpt],
    body: &Body,
) -> anyhow::Result<Vec<vsmtp_common::rcpt::Rcpt>> {
    // we pickup a copy of the metadata and envelop of the context, so we can dispatch emails
    // to send by groups of recipients (grouped by transfer method)

    // filtering recipients by domains and delivery method.
    let mut triage = vsmtp_common::rcpt::filter_by_transfer_method(to);

    // getting a raw copy of the email.
    let content = match &body {
        Body::Empty => anyhow::bail!(
            "empty body found in message '{}' in delivery queue",
            metadata.message_id
        ),
        Body::Raw(raw) => raw.clone(),
        Body::Parsed(parsed) => parsed.to_raw(),
    };

    for (method, rcpt) in &mut triage {
        let mut transport: Box<dyn vsmtp_delivery::transport::Transport + Send> = match method {
            vsmtp_common::transfer::Transfer::Forward(to) => {
                Box::new(vsmtp_delivery::transport::forward::Forward(to.clone()))
            }
            vsmtp_common::transfer::Transfer::Deliver => {
                Box::new(vsmtp_delivery::transport::deliver::Deliver)
            }
            vsmtp_common::transfer::Transfer::Mbox => {
                Box::new(vsmtp_delivery::transport::mbox::MBox)
            }
            vsmtp_common::transfer::Transfer::Maildir => {
                Box::new(vsmtp_delivery::transport::maildir::Maildir)
            }
            vsmtp_common::transfer::Transfer::None => continue,
        };

        transport
            .deliver(config, dns, metadata, from, &mut rcpt[..], &content)
            .await
            .with_context(|| {
                format!("failed to deliver email using '{method}' for group '{rcpt:?}'")
            })?;
    }

    // recipient email transfer status could have been updated.
    Ok(triage
        .into_iter()
        .flat_map(|(_, rcpt)| rcpt)
        // filter out recipients if they have been sent the message already.
        .filter(|rcpt| !matches!(rcpt.email_status, EmailTransferStatus::Sent))
        .collect::<Vec<_>>())
}

/// copy the message into the deferred / dead queue if any recipient is held back or have failed delivery.
fn move_to_queue(
    config: &Config,
    message_id: &str,
    path: &std::path::Path,
    rcpt: &[vsmtp_common::rcpt::Rcpt],
) -> anyhow::Result<()> {
    if rcpt.iter().any(|rcpt| {
        matches!(
            rcpt.email_status,
            vsmtp_common::transfer::EmailTransferStatus::HeldBack(..)
        )
    }) {
        std::fs::copy(
            path,
            std::path::PathBuf::from_iter([
                Queue::Deferred.to_path(&config.server.queues.dirpath)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )
        .context("failed to move message from delivery queue to deferred queue")?;
    }

    if rcpt.iter().any(|rcpt| {
        matches!(
            rcpt.email_status,
            vsmtp_common::transfer::EmailTransferStatus::Failed(..)
        )
    }) {
        std::fs::copy(
            path,
            std::path::PathBuf::from_iter([
                Queue::Dead.to_path(&config.server.queues.dirpath)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )
        .context("failed to move message from delivery queue to dead queue")?;
    }

    Ok(())
}

/// prepend trace informations to headers.
/// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.4
fn add_trace_information(
    config: &Config,
    ctx: &mut MailContext,
    rule_engine_result: Status,
) -> anyhow::Result<()> {
    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing email metadata"))?;

    let stamp = create_received_stamp(
        &ctx.envelop.helo,
        &config.server.domain,
        &metadata.message_id,
        &metadata.timestamp,
    )
    .context("failed to create Receive header timestamp")?;

    let vsmtp_status = create_vsmtp_status_stamp(
        &ctx.metadata.as_ref().unwrap().message_id,
        &config.version_requirement.to_string(),
        rule_engine_result,
    );

    match &mut ctx.body {
        Body::Empty => {
            anyhow::bail!("could not add trace information to email header: body is empty")
        }
        Body::Raw(raw) => {
            *raw = format!("Received: {}\nX-VSMTP: {}\n{}", stamp, vsmtp_status, raw);
        }
        Body::Parsed(parsed) => {
            parsed.prepend_headers(vec![
                ("Received".to_string(), stamp),
                ("X-VSMTP".to_string(), vsmtp_status),
            ]);
        }
    };

    Ok(())
}

/// create the "Received" header stamp.
fn create_received_stamp(
    client_helo: &str,
    server_domain: &str,
    message_id: &str,
    received_timestamp: &std::time::SystemTime,
) -> anyhow::Result<String> {
    Ok(format!(
        "from {client_helo}\n\tby {server_domain}\n\twith SMTP\n\tid {message_id};\n\t{}",
        {
            let odt: time::OffsetDateTime = (*received_timestamp).into();

            odt.format(&Rfc2822)?
        }
    ))
}

/// create the "X-VSMTP" header stamp.
fn create_vsmtp_status_stamp(message_id: &str, version: &str, status: Status) -> String {
    format!(
        "id='{}'\n\tversion='{}'\n\tstatus='{}'",
        message_id, version, status
    )
}
