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
    queue::Queue,
    rules::rule_engine::{RuleEngine, Status},
};

use super::ProcessMessage;

/// process that treats incoming email offline with the postq stage.
pub async fn start(
    config: &ServerConfig,
    mut working_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    async fn handle_one(
        process_message: ProcessMessage,
        config: &ServerConfig,
        delivery_sender: &tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<()> {
        log::debug!(
            target: DELIVER,
            "vMIME process received a new message id: {}",
            process_message.message_id,
        );

        let working_queue = Queue::Working.to_path(config.smtp.spool_dir.clone())?;
        let file_to_process = working_queue.join(&process_message.message_id);

        log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

        let mut ctx: crate::model::mail::MailContext =
            serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

        let parsed_email = match &ctx.body {
            Body::Parsed(parsed_email) => parsed_email.clone(),
            Body::Raw(raw) => Box::new(MailMimeParser::default().parse(raw.as_bytes())?),
        };

        let mut rule_engine = RuleEngine::new(config);

        // TODO: add connection data.
        rule_engine
            .add_data("helo", ctx.envelop.helo.clone())
            .add_data("mail", ctx.envelop.mail_from.clone())
            .add_data("rcpts", ctx.envelop.rcpt.clone())
            .add_data("data", *parsed_email)
            .add_data("metadata", ctx.metadata.clone());

        match rule_engine.run_when("postq") {
            Status::Deny => Queue::Dead.write_to_queue(config, &ctx)?,
            Status::Block => Queue::Quarantine.write_to_queue(config, &ctx)?,
            _ => {
                match rule_engine.get_scoped_envelop() {
                    Some((envelop, metadata, mail)) => {
                        ctx.envelop = envelop;
                        ctx.metadata = metadata;
                        ctx.body = Body::Parsed(mail.into());
                    }
                    _ => anyhow::bail!(
                        "one of the email context variables could not be found in rhai's context."
                    ),
                };

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

                delivery_sender
                    .send(ProcessMessage {
                        message_id: process_message.message_id.to_string(),
                    })
                    .await
                    .unwrap();

                std::fs::remove_file(&file_to_process)?;

                log::debug!(
                    target: DELIVER,
                    "message '{}' removed from working queue.",
                    process_message.message_id
                );
            }
        };

        Ok(())
    }

    loop {
        if let Some(pm) = working_receiver.recv().await {
            handle_one(pm, config, &delivery_sender).await.unwrap();
        }
    }
}
