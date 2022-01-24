use crate::config::{log_channel::RECEIVER, server_config::ServerConfig};

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

/// identifiers for all mail queues.
pub enum Queue {
    Working,
    Deliver,
    Deferred,
    Dead,
}

impl Queue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Queue::Working => "working",
            Queue::Deliver => "deliver",
            Queue::Deferred => "deferred",
            Queue::Dead => "dead",
        }
    }

    pub fn to_path(
        &self,
        parent: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<std::path::PathBuf> {
        let dir = parent.into().join(self.as_str());
        if !dir.exists() {
            std::fs::DirBuilder::new().recursive(true).create(&dir)?;
        }
        Ok(dir)
    }

    /// write the email to a queue and send the message id to another process.
    pub async fn write_to_queue(
        &self,
        sender: &tokio::sync::mpsc::Sender<String>,
        config: &ServerConfig,
        ctx: &crate::model::mail::MailContext,
    ) -> Result<(), std::io::Error> {
        let to_deliver = self
            .to_path(&config.smtp.spool_dir)?
            .join(&ctx.metadata.as_ref().unwrap().message_id);

        // TODO: should loop if a file name is conflicting.
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&to_deliver)?;

        std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())?;

        let message_id = ctx.metadata.as_ref().unwrap().message_id.clone();

        log::trace!(
            target: RECEIVER,
            "mail {} successfully written to {} queue",
            message_id,
            self.as_str()
        );

        // TODO: handle send errors.
        // sending the message id to the delivery process.
        // NOTE: we could send the context instead, so that the delivery system won't have
        //       to touch the file system.
        sender.send(message_id).await.unwrap();

        Ok(())
    }
}
