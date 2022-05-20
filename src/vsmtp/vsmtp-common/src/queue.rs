/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::mail_context::MailContext;
use anyhow::Context;

/// identifiers for all mail queues.
#[derive(Debug, PartialEq, Copy, Clone, strum::Display, strum::EnumString, strum::EnumIter)]
#[strum(serialize_all = "lowercase")]
pub enum Queue {
    /// postq
    Working,
    /// 1st attempt to deliver
    Deliver,
    /// delivery #1 failed, next attempts
    Deferred,
    /// too many attempts failed
    Dead,
}

/// Syntax sugar for access of queues folder and queues items
///
/// # Errors
///
/// * if [`create_if_missing`] is provided, will attempt to create the folder
#[macro_export]
macro_rules! queue_path {
    ($queues_dirpath:expr, $queue:expr) => {
        std::path::PathBuf::from($queues_dirpath).join(format!("{}", $queue))
    };
    ($queues_dirpath:expr, $queue:expr, $msg_id:expr) => {
        $crate::queue_path!($queues_dirpath, $queue).join($msg_id)
    };

    (create_if_missing => $queues_dirpath:expr, $queue:expr) => {
        {
            let buf = std::path::PathBuf::from($queues_dirpath).join(format!("{}", $queue));
            if !buf.exists() {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .create(&buf).map(|_| buf)
            } else {
                std::io::Result::Ok(buf)
            }
        }
    };
    (create_if_missing => $queues_dirpath:expr, $queue:expr, $msg_id:expr) => {
        $crate::queue_path!(create_if_missing => $queues_dirpath, $queue).map(|buf| buf.join($msg_id))
    };
}

impl Queue {
    /// List the files contained in the queue
    ///
    /// # Errors
    ///
    /// * failed to initialize queue
    /// * error while reading directory
    /// * one entry produced an error
    pub fn list_entries(
        &self,
        queues_dirpath: &std::path::Path,
    ) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let queue_path = queue_path!(queues_dirpath, self);

        queue_path
            .read_dir()
            .context(format!("Error from read dir '{}'", queue_path.display()))?
            .map(|e| match e {
                Ok(e) => Ok(e.path()),
                Err(e) => Err(anyhow::Error::new(e)),
            })
            .collect::<anyhow::Result<Vec<_>>>()
    }

    /// Write a [`MailContext`] to the [`self`] queue
    ///
    /// # Errors
    ///
    /// * the message's metadata is ill-formed
    /// * errors from [`Self::to_path`]
    /// * failed to serialize the `@ctx`
    /// * failed to write on `@ctx` on `queues_dirpath/self/ctx.id`
    pub fn write_to_queue(
        &self,
        queues_dirpath: &std::path::Path,
        ctx: &MailContext,
    ) -> anyhow::Result<()> {
        let message_id = match ctx.metadata.as_ref() {
            Some(metadata) => &metadata.message_id,
            None => anyhow::bail!("could not write to {} queue: mail metadata not found", self),
        };

        let to_deliver = queue_path!(create_if_missing => queues_dirpath, self, message_id)?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&to_deliver)
            .with_context(|| {
                format!("failed to open file in {} queue at {:?}", self, to_deliver)
            })?;

        std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())?;

        log::debug!("mail {} successfully written to {} queue", message_id, self);

        Ok(())
    }
}
