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
use anyhow::Context;
use vsmtp_common::mail_context::MailContext;
use vsmtp_config::{log_channel::RECEIVER, Config};

/// identifiers for all mail queues.
pub(crate) enum Queue {
    /// postq
    Working,
    /// 1st attempt to deliver
    Deliver,
    /// delivery #1 failed, next attempts
    Deferred,
    /// too many attempts failed
    Dead,
    /// user defined queue
    #[allow(unused)]
    Quarantine,
}

impl Queue {
    pub const fn as_str(&self) -> &'static str {
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

    pub fn write_to_queue(&self, config: &Config, ctx: &MailContext) -> anyhow::Result<()> {
        let message_id = match ctx.metadata.as_ref() {
            Some(metadata) => &metadata.message_id,
            None => anyhow::bail!(
                "could not write to {} queue: mail metadata not found",
                self.as_str()
            ),
        };

        let to_deliver = self
            .to_path(&config.server.queues.dirpath)?
            .join(message_id);

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
