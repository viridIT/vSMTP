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
) -> std::io::Result<()> {
    async fn handle_one(
        process_message: ProcessMessage,
        config: &ServerConfig,
        delivery_sender: &tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> std::io::Result<()> {
        log::debug!(
            target: DELIVER,
            "vMIME process received a new message id: {}",
            process_message.message_id,
        );

        let working_queue = Queue::Working.to_path(config.smtp.spool_dir.clone())?;
        let file_to_process = working_queue.join(&process_message.message_id);

        log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

        let mail: crate::model::mail::MailContext =
            serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

        let parsed_email = match &mail.body {
            Body::Parsed(parsed_email) => parsed_email.clone(),
            Body::Raw(raw) => Box::new(
                MailMimeParser::default()
                    .parse(raw.as_bytes())
                    .expect("handle errors when parsing email in vMIME"),
            ),
        };

        let mut rule_engine = RuleEngine::new(config);

        rule_engine
            .add_data("data", parsed_email)
            .add_data("helo", mail.envelop.helo.clone())
            .add_data("mail", mail.envelop.mail_from.clone())
            .add_data("metadata", mail.metadata.clone())
            .add_data("rcpts", mail.envelop.rcpt.clone());

        match rule_engine.run_when("postq") {
            Status::Deny => {
                todo!("denied in postq")
            }
            Status::Block => {
                todo!("blocked in postq")
            }
            _ => {
                let mut to_deliver = std::fs::OpenOptions::new().create(true).write(true).open(
                    std::path::PathBuf::from_iter([
                        Queue::Deliver.to_path(&config.smtp.spool_dir)?,
                        std::path::Path::new(&process_message.message_id).to_path_buf(),
                    ]),
                )?;

                std::io::Write::write_all(
                    &mut to_deliver,
                    serde_json::to_string(&mail)?.as_bytes(),
                )?;

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
