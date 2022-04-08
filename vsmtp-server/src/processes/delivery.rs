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
    channel_message::ProcessMessage,
    processes::delivery::{
        deferred::flush_deferred_queue,
        deliver::{flush_deliver_queue, handle_one_in_delivery_queue},
    },
    queue::Queue,
};
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::{Body, MailContext},
    re::{anyhow, log},
    status::Status,
    transfer::{EmailTransferStatus, Transfer},
};
use vsmtp_config::{log_channel::DELIVER, Config};
use vsmtp_delivery::transport::{deliver as deliver2, forward, maildir, mbox, Transport};
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod deferred;
mod deliver;

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

    let dns = std::sync::Arc::new(
        vsmtp_config::build_dns(&config).context("could not initialize the delivery dns")?,
    );

    flush_deliver_queue(&config, &dns, &rule_engine).await?;

    let mut flush_deferred_interval =
        tokio::time::interval(config.server.queues.delivery.deferred_retry_period);

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                // FIXME: transports are mutable, so must be in a mutex
                // for a delivery in a separated thread...
                let copy_config = config.clone();
                let copy_rule_engine = rule_engine.clone();
                let copy_dns = dns.clone();
                tokio::spawn(async move {
                    let path = match Queue::Deliver.to_path(&copy_config.server.queues.dirpath) {
                        Ok(path) => path,
                        Err(_) => return // todo : log no file
                    };

                    if let Err(error) = handle_one_in_delivery_queue(
                        &copy_config,
                        &copy_dns,
                        &std::path::PathBuf::from_iter([
                            path,
                            std::path::Path::new(&pm.message_id).to_path_buf(),
                        ]),
                        &copy_rule_engine,
                    )
                    .await {
                        log::error!(target: DELIVER, "could not deliver email '{}': {error:?}", pm.message_id);
                    }
                });

                if cfg!(test) {
                    return Ok(());
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
        let mut transport: Box<dyn Transport + Send> = match method {
            Transfer::Forward(to) => Box::new(forward::Forward(to.clone())),
            Transfer::Deliver => Box::new(deliver2::Deliver),
            Transfer::Mbox => Box::new(mbox::MBox),
            Transfer::Maildir => Box::new(maildir::Maildir),
            Transfer::None => continue,
        };

        transport
            .deliver(config, dns, metadata, from, &mut rcpt[..], &content)
            .await
            .with_context(|| {
                format!("failed to deliver email using '{method}' for group '{rcpt:?}'")
            })?;
    }

    // recipient email transfer status could have been updated.
    // we also filter out recipients if they have been sent the message already.
    Ok(triage
        .into_iter()
        .flat_map(|(_, rcpt)| rcpt)
        .filter(|rcpt| !matches!(rcpt.email_status, EmailTransferStatus::Sent))
        .collect::<Vec<_>>())
}

/// copy the message into the deferred / dead queue if any recipient is held back or have failed delivery.
/// FIXME: could be optimized by checking both conditions with the same iterator.
fn move_to_queue(config: &Config, ctx: &MailContext) -> anyhow::Result<()> {
    if ctx
        .envelop
        .rcpt
        .iter()
        .any(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
    {
        Queue::Deferred
            .write_to_queue(config, ctx)
            .context("failed to move message from delivery queue to deferred queue")?;
    }

    if ctx.envelop.rcpt.iter().any(|rcpt| {
        matches!(rcpt.email_status, EmailTransferStatus::Failed(..))
            || matches!(rcpt.transfer_method, Transfer::None,)
    }) {
        Queue::Dead
            .write_to_queue(config, ctx)
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

#[cfg(test)]
mod test {
    use super::add_trace_information;
    use crate::ProcessMessage;
    use vsmtp_common::mail_context::Body;
    use vsmtp_rule_engine::rule_engine::RuleEngine;
    use vsmtp_test::config;

    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn start() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();
        config.app.vsl.filepath = "./src/tests/empty_main.vsl".into();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::new(&Some(config.app.vsl.filepath.clone())).unwrap(),
        ));

        let (delivery_sender, delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let task = tokio::spawn(super::start(
            std::sync::Arc::new(config),
            rule_engine,
            delivery_receiver,
        ));

        delivery_sender
            .send(ProcessMessage {
                message_id: "test".to_string(),
            })
            .await
            .unwrap();

        task.await.unwrap().unwrap();
    }

    #[test]
    fn test_add_trace_information() {
        let mut ctx = vsmtp_common::mail_context::MailContext {
            body: vsmtp_common::mail_context::Body::Empty,
            connection_timestamp: std::time::SystemTime::UNIX_EPOCH,
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop {
                helo: "localhost".to_string(),
                mail_from: vsmtp_common::address::Address::try_from("a@a.a".to_string()).unwrap(),
                rcpt: vec![],
            },
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::UNIX_EPOCH,
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        };

        let config = vsmtp_config::Config::default();

        if let Err(error) =
            add_trace_information(&config, &mut ctx, vsmtp_common::status::Status::Next)
        {
            assert_eq!(
                &error.to_string(),
                "could not add trace information to email header: body is empty"
            );
        } else {
            panic!("add_trace_information did not return an error on empty body");
        }

        ctx.body = Body::Raw("".to_string());
        ctx.metadata.as_mut().unwrap().message_id = "test_message_id".to_string();
        add_trace_information(&config, &mut ctx, vsmtp_common::status::Status::Next).unwrap();

        assert_eq!(
            match &ctx.body {
                Body::Raw(raw) => raw,
                _ => unreachable!(),
            },
            &format!(
                "Received: from localhost\n\tby {}\n\twith SMTP\n\tid {};\n\t{}\nX-VSMTP: id='{}'\n\tversion='{}'\n\tstatus='next'\n",
                config.server.domain,
                ctx.metadata.as_ref().unwrap().message_id,
                {
                    let odt: time::OffsetDateTime = ctx.metadata.as_ref().unwrap().timestamp.into();

                    odt.format(&time::format_description::well_known::Rfc2822).unwrap()
                },
                ctx.metadata.as_ref().unwrap().message_id,
                config.version_requirement,
            )
        );
    }
}
