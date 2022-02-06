/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::{
    config::{log_channel::DELIVER, server_config::ServerConfig},
    mime::parser::MailMimeParser,
    model::mail::Body,
    processes::ProcessMessage,
    queue::Queue,
    rules::rule_engine::{RuleEngine, RuleState, Status},
};

use anyhow::Context;

/// process that treats incoming email offline with the postq stage.
pub async fn start(
    config: &ServerConfig,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    loop {
        if let Some(pm) = working_receiver.recv().await {
            if let Err(err) =
                handle_one_in_working_queue(config, &rule_engine, pm, &delivery_sender).await
            {
                log::error!("{}", err);
            }
        }
    }
}

pub(crate) async fn handle_one_in_working_queue(
    config: &ServerConfig,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    process_message: ProcessMessage,
    delivery_sender: &tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    log::debug!(
        target: DELIVER,
        "vMIME process received a new message id: {}",
        process_message.message_id,
    );

    let file_to_process = Queue::Working
        .to_path(&config.delivery.spool_dir)?
        .join(&process_message.message_id);

    log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

    let mut ctx: crate::model::mail::MailContext =
        serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

    if let Body::Raw(raw) = &ctx.body {
        ctx.body = Body::Parsed(Box::new(MailMimeParser::default().parse(raw.as_bytes())?));
    }

    let mut state = RuleState::new(config);
    state.add_data("ctx", ctx);
    let result = rule_engine.read().unwrap().run_when(&mut state, "postq");

    match result {
        Status::Deny => Queue::Dead.write_to_queue(config, &state.get_context().read().unwrap())?,
        Status::Block => Queue::Quarantine.write_to_queue(config, &state.get_context().read().unwrap())?,
        _ => {
            {
                let ctx = state.get_context();
                let ctx = ctx.read().unwrap();
                match &ctx.metadata {
                    // quietly skipping delivery processes when there is no resolver.
                    // (in case of a quarantine for example)
                    Some(metadata) if metadata.resolver == "none" => {
                        log::warn!(
                            target: DELIVER,
                            "delivery skipped due to NO_DELIVERY action call."
                        );
                        return Ok(());
                    }
                    _ => {}
                };

                Queue::Deliver.write_to_queue(config, &ctx)?;
            }

            delivery_sender
                .send(ProcessMessage {
                    message_id: process_message.message_id.to_string(),
                })
                .await?;

            std::fs::remove_file(&file_to_process)
                .context("failed to remove a file from the working queue")?;

            log::debug!(
                target: DELIVER,
                "message '{}' removed from working queue.",
                process_message.message_id
            );
        }
    };

    Ok(())
}
