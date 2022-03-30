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
use crate::{processes::ProcessMessage, queue::Queue};
use anyhow::Context;
use vsmtp_common::{
    mail_context::{Body, MailContext},
    re::{anyhow, log},
    state::StateSMTP,
    status::Status,
};
use vsmtp_config::{log_channel::DELIVER, Config};
use vsmtp_mail_parser::MailMimeParser;
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

/// process that treats incoming email offline with the postq stage.
///
/// # Errors
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    loop {
        if let Some(pm) = working_receiver.recv().await {
            if let Err(err) = tokio::spawn(handle_one_in_working_queue(
                config.clone(),
                rule_engine.clone(),
                pm,
                delivery_sender.clone(),
            ))
            .await
            {
                log::error!("{}", err);
            }
        }
    }
}

///
/// # Errors
///
/// # Panics
pub async fn handle_one_in_working_queue(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    process_message: ProcessMessage,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    log::debug!(
        target: DELIVER,
        "vMIME process received a new message id: {}",
        process_message.message_id,
    );

    let file_to_process = Queue::Working
        .to_path(&config.server.queues.dirpath)?
        .join(&process_message.message_id);

    log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

    let mut ctx = MailContext::from_file(&file_to_process).with_context(|| {
        format!(
            "failed to deserialize email '{}'",
            &process_message.message_id
        )
    })?;

    if let Body::Raw(raw) = &ctx.body {
        ctx.body = Body::Parsed(Box::new(MailMimeParser::default().parse(raw.as_bytes())?));
    }
    let mut state = RuleState::with_context(config.as_ref(), ctx);

    let result = rule_engine
        .read()
        .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?
        .run_when(&mut state, &StateSMTP::PostQ);

    if result == Status::Deny {
        Queue::Dead.write_to_queue(config.as_ref(), &state.get_context().read().unwrap())?;
    } else {
        {
            let ctx = state.get_context();
            let ctx = ctx.read().unwrap();

            if ctx
                .envelop
                .rcpt
                .iter()
                .all(|rcpt| rcpt.transfer_method == vsmtp_common::transfer::Transfer::None)
            {
                // skipping mime & delivery processes.
                log::warn!(
                    target: DELIVER,
                    "delivery skipped because all recipient's transfer method is set to None."
                );
                Queue::Dead.write_to_queue(config.as_ref(), &ctx)?;
                return Ok(());
            }
        }

        delivery_sender
            .send(ProcessMessage {
                message_id: process_message.message_id.to_string(),
            })
            .await?;

        anyhow::Context::context(
            std::fs::remove_file(&file_to_process),
            "failed to remove a file from the working queue",
        )?;

        log::debug!(
            target: DELIVER,
            "message '{}' removed from working queue.",
            process_message.message_id
        );
    };

    Ok(())
}
