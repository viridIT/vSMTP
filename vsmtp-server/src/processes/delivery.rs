/**
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
 **/
use super::ProcessMessage;
use crate::queue::Queue;
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
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
pub async fn start<S: std::hash::BuildHasher + Send>(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut transports: std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) -> anyhow::Result<()> {
    log::info!(
        target: DELIVER,
        "vDeliver (delivery) booting, flushing queue.",
    );
    flush_deliver_queue(&config, &rule_engine, &mut transports).await?;

    let mut flush_deferred_interval =
        tokio::time::interval(config.server.queues.delivery.deferred_retry_period);

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                // FIXME: transports are mutable, so must be in a mutex
                // for a delivery in a separated thread...
                if let Err(error) = handle_one_in_delivery_queue(
                    &config,
                    &pm.message_id,
                    &std::path::PathBuf::from_iter([
                        Queue::Deliver.to_path(&config.server.queues.dirpath)?,
                        std::path::Path::new(&pm.message_id).to_path_buf(),
                    ]),
                    &rule_engine,
                    &mut transports,
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
                flush_deferred_queue(&mut transports, &config).await?;
            }
        };
    }
}

/// handle one email pulled from the delivery queue.
///
/// # Panics
///
/// # Errors
pub async fn handle_one_in_delivery_queue<S: std::hash::BuildHasher + Send>(
    config: &Config,
    message_id: &str,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    transports: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
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

    // NOTE: should the engine able to return a status for a particular recipient ?
    if result == Status::Deny {
        // we update rcpt email status and write to dead queue in case of a deny.
        let ctx = state.get_context();
        let mut ctx = ctx.write().unwrap();

        add_trace_information(&mut ctx, config, result)?;

        for rcpt in &mut ctx.envelop.rcpt {
            rcpt.email_status =
                EmailTransferStatus::Failed("rule engine denied the email.".to_string());
        }
        Queue::Dead.write_to_queue(config, &ctx)?;
    } else {
        // we pickup a copy of the metadata and envelop of the context, so we can dispatch emails
        // to send by groups of recipients (grouped by transfer + destination)
        // NOTE: using a lambda here because extraction can fail & locking ctx needs to be scoped
        //       because of the async code below.
        let (from, mut triage, content, metadata) =
            (|state: &mut RuleState| -> anyhow::Result<_> {
                let ctx = state.get_context();
                let mut ctx = ctx.write().unwrap();

                add_trace_information(&mut ctx, config, result)?;

                println!("{:#?}", *ctx);

                // filtering recipients by domains and delivery method.
                let triage = filter_recipients(&*ctx, transports);

                // getting a raw copy of the email.
                let content = match &ctx.body {
                    Body::Empty => todo!("an empty body should not be possible in delivery"),
                    Body::Raw(raw) => raw.clone(),
                    Body::Parsed(parsed) => parsed.to_raw(),
                };

                let metadata = ctx
                    .metadata
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing email metadata"))?
                    .clone();

                Ok((ctx.envelop.mail_from.clone(), triage, content, metadata))
            })(&mut state)?;

        for (method, rcpt) in &mut triage {
            println!("'{method}' for '{rcpt:?}'");
            transports
                .get_mut(method)
                .unwrap()
                .deliver(config, &metadata, &from, &mut rcpt[..], &content)
                .await
                .with_context(|| {
                    format!("failed to deliver email using '{method}' for '{rcpt:?}'")
                })?;
        }

        let ctx = state.get_context();
        let mut ctx = ctx.write().unwrap();

        // recipient email transfer status could have been updated.
        ctx.envelop.rcpt = triage.into_iter().flat_map(|(_, rcpt)| rcpt).collect();

        // FIXME: disk i/o could be avoided here by filtering rcpt statuses.
        for rcpt in &mut ctx.envelop.rcpt {
            match &rcpt.email_status {
                vsmtp_common::transfer::EmailTransferStatus::HeldBack(_) => {
                    std::fs::rename(
                        path,
                        std::path::PathBuf::from_iter([
                            Queue::Deferred
                                .to_path(&config.server.queues.dirpath)
                                .unwrap(),
                            std::path::Path::new(&message_id).to_path_buf(),
                        ]),
                    )
                    .unwrap();
                }
                vsmtp_common::transfer::EmailTransferStatus::Failed(_) => std::fs::rename(
                    path,
                    std::path::PathBuf::from_iter([
                        Queue::Dead.to_path(&config.server.queues.dirpath).unwrap(),
                        std::path::Path::new(&message_id).to_path_buf(),
                    ]),
                )
                .unwrap(),
                // Sent or Waiting (waiting should never happen), we can remove the file later.
                _ => {}
            }
        }
    };

    // after processing the email is removed from the delivery queue.
    std::fs::remove_file(path)?;

    Ok(())
}

async fn flush_deliver_queue<S: std::hash::BuildHasher + Send>(
    config: &Config,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.server.queues.dirpath)?)? {
        let path = path.context("could not flush delivery queue")?;
        let message_id = path.file_name();

        handle_one_in_delivery_queue(
            config,
            message_id
                .to_str()
                .context("could not fetch message id in delivery queue")?,
            &path.path(),
            rule_engine,
            resolvers,
        )
        .await?;
    }

    Ok(())
}

// NOTE: emails stored in the deferred queue are lickly to slow down the process.
//       the pickup process of this queue should be slower than pulling from the delivery queue.
//       https://www.postfix.org/QSHAPE_README.html#queues
async fn handle_one_in_deferred_queue<S: std::hash::BuildHasher + Send>(
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
    path: &std::path::Path,
    config: &Config,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::debug!(
        target: DELIVER,
        "vDeliver (deferred) received email '{}'",
        message_id
    );

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

    let mut raw =
        String::with_capacity(usize::try_from(file.metadata().unwrap().len()).unwrap_or(0));
    std::io::Read::read_to_string(&mut file, &mut raw)?;

    let ctx: MailContext = serde_json::from_str(&raw)?;

    let max_retry_deferred = config.server.queues.delivery.deferred_retry_max;

    if ctx.metadata.is_none() {
        anyhow::bail!("email metadata is missing")
    }

    // for rcpt in &mut ctx.envelop.rcpt {
    //     if rcpt.retry >= max_retry_deferred {
    //         // TODO: move to dead queue.
    //         continue;
    //     }

    //     let resolver = if let Some(resolver) = resolvers.get_mut(&rcpt.transfer_method) {
    //         resolver
    //     } else {
    //         log::trace!(
    //             target: DELIVER,
    //             "vDeliver (deferred) delivery method '{}' for '{}' not found",
    //             rcpt.transfer_method,
    //             rcpt.address
    //         );

    //         // TODO: set in dead.

    //         continue;
    //     };

    //     match resolver.deliver(config, &ctx, rcpt).await {
    //         Ok(_) => {
    //             log::debug!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' email sent successfully.",
    //                 message_id
    //             );

    //             std::fs::remove_file(&path)?;
    //         }
    //         Err(error) => {
    //             log::warn!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' failed to send email, reason: '{}'",
    //                 message_id,
    //                 error
    //             );

    //             rcpt.retry += 1;

    //             let mut file = std::fs::OpenOptions::new()
    //                 .truncate(true)
    //                 .write(true)
    //                 .open(&path)
    //                 .unwrap();

    //             std::io::Write::write_all(&mut file, serde_json::to_string(&ctx)?.as_bytes())?;

    //             log::debug!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' increased retries to '{}'.",
    //                 message_id,
    //                 rcpt.retry
    //             );
    //         }
    //     }
    // }

    Ok(())
}

async fn flush_deferred_queue<S: std::hash::BuildHasher + Send>(
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
    config: &Config,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deferred.to_path(&config.server.queues.dirpath)?)? {
        handle_one_in_deferred_queue(resolvers, &path?.path(), config).await?;
    }

    Ok(())
}

/// filter recipients by their transfer method.
/// the context is mutable because transports could not be correctly setup.
/// FIXME: find a better to couple Transfer methods with Transport.
///        that way, the email status would never be failed at this stage.
fn filter_recipients<S: std::hash::BuildHasher + Send>(
    ctx: &MailContext,
    transports: &std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn vsmtp_delivery::transport::Transport + Send + Sync>,
        S,
    >,
) -> std::collections::HashMap<vsmtp_common::transfer::Transfer, Vec<vsmtp_common::rcpt::Rcpt>> {
    ctx.envelop
        .rcpt
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, rcpt| {
            let mut rcpt = rcpt.clone();
            if !transports.contains_key(&rcpt.transfer_method) {
                rcpt.email_status = vsmtp_common::transfer::EmailTransferStatus::Failed(format!(
                    "{} transfer method does not have a transfer system setup",
                    rcpt.transfer_method.as_str()
                ));
                return acc;
            }

            if let Some(group) = acc.get_mut(&rcpt.transfer_method) {
                group.push(rcpt);
            } else {
                acc.insert(rcpt.transfer_method, vec![rcpt]);
            }

            acc
        })
}

/// prepend trace informations to headers.
/// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.4
// TODO: add Return-Path header.
fn add_trace_information(
    ctx: &mut MailContext,
    config: &Config,
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

// NOTE: should this function moved to the email parser library ?
/// create the "Received" header.
fn create_received_stamp(
    client_helo: &str,
    server_domain: &str,
    message_id: &str,
    received_timestamp: &std::time::SystemTime,
) -> anyhow::Result<String> {
    // NOTE: after "for": potential Additional-Registered-Clauses
    Ok(format!(
        "from {client_helo}\n\tby {server_domain}\n\twith SMTP\n\tid {message_id};\n\t{}",
        {
            let odt: time::OffsetDateTime = (*received_timestamp).into();

            odt.format(&Rfc2822)?
        }
    ))
}

// NOTE: should this function moved to the email parser library ?
/// create the "X-VSMTP" header.
fn create_vsmtp_status_stamp(message_id: &str, version: &str, status: Status) -> String {
    format!(
        "id='{}'\n\tversion='{}'\n\tstatus='{}'",
        message_id, version, status
    )
}
