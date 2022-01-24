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
    config::log_channel::DELIVER, mime::parser::MailMimeParser, model::mail::Body, queue::Queue,
};

/// process that treats incoming email offline with the postq stage.
pub async fn start(
    spool_dir: String,
    mut working_receiver: tokio::sync::mpsc::Receiver<String>,
    delivery_sender: tokio::sync::mpsc::Sender<String>,
) -> std::io::Result<()> {
    async fn handle_one(
        message_id: &str,
        spool_dir: &str,
        delivery_sender: &tokio::sync::mpsc::Sender<String>,
    ) -> std::io::Result<()> {
        log::debug!(
            target: DELIVER,
            "vMIME process received a new message id: {}",
            message_id
        );

        let working_queue = Queue::Working.to_path(spool_dir)?;

        let file_to_process = working_queue.join(&message_id);
        log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

        let mail: crate::model::mail::MailContext =
            serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

        match mail.body {
            Body::Parsed(_) => {}
            Body::Raw(ref raw) => {
                MailMimeParser::default()
                    .parse(raw.as_bytes())
                    // .and_then(|_| todo!("run postq rule engine"))
                    .expect("handle errors when parsing email in vMIME");
            }
        };

        // TODO: run postq rule engine.

        let mut to_deliver = std::fs::OpenOptions::new().create(true).write(true).open(
            std::path::PathBuf::from_iter([
                Queue::Deliver.to_path(&spool_dir)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )?;

        std::io::Write::write_all(&mut to_deliver, serde_json::to_string(&mail)?.as_bytes())?;

        delivery_sender.send(message_id.to_string()).await.unwrap();

        std::fs::remove_file(&file_to_process)?;

        log::debug!(
            target: DELIVER,
            "message '{}' removed from working queue.",
            message_id
        );

        Ok(())
    }

    loop {
        if let Some(message_id) = working_receiver.recv().await {
            handle_one(&message_id, &spool_dir, &delivery_sender)
                .await
                .unwrap();
        }
    }
}
