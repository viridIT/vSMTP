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
use anyhow::Context;

use crate::config::{log_channel::RECEIVER, server_config::ServerConfig};

/// identifiers for all mail queues.
pub enum Queue {
    Working,
    Deliver,
    Deferred,
    Dead,
    Quarantine,
}

impl Queue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Queue::Working => "working",
            Queue::Deliver => "deliver",
            Queue::Deferred => "deferred",
            Queue::Dead => "dead",
            Queue::Quarantine => "quarantine",
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

    pub fn write_to_queue(
        &self,
        config: &ServerConfig,
        ctx: &crate::model::mail::MailContext,
    ) -> anyhow::Result<()> {
        let message_id = match ctx.metadata.as_ref() {
            Some(metadata) => &metadata.message_id,
            None => anyhow::bail!("mail metadata not found"),
        };

        let to_deliver = self.to_path(&config.smtp.spool_dir)?.join(message_id);

        // TODO: should loop if a file name is conflicting.
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&to_deliver)
            .with_context(|| {
                format!(
                    "failed to open file in {} queue at {:?}",
                    self.as_str(),
                    to_deliver,
                )
            })?;

        std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())?;

        log::debug!(
            target: RECEIVER,
            "mail {} successfully written to {} queue",
            message_id,
            self.as_str()
        );

        Ok(())
    }
}
