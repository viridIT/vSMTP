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
use crate::mail_context::{MailContext, MessageBody};
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
    ) -> std::io::Result<()> {
        let message_id = &ctx
            .metadata
            .as_ref()
            .expect("not ill-formed mail context")
            .message_id;

        let to_deliver = queue_path!(create_if_missing => queues_dirpath, self, message_id)?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&to_deliver)?;

        std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())?;

        log::debug!("mail {message_id} successfully written to {self} queue");

        Ok(())
    }

    /// Write a [`MessageBody`] to path provided
    ///
    /// # Errors
    ///
    /// * failed to open file
    /// * failed to serialize the `mail`
    /// * failed to write the `mail` on `path`
    pub async fn write_to_quarantine(
        path: &std::path::Path,
        mail: &MailContext,
    ) -> std::io::Result<()> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        let serialized = serde_json::to_string(mail)?;

        tokio::io::AsyncWriteExt::write_all(&mut file, serialized.as_bytes()).await
    }

    ///
    /// # Errors
    ///
    /// * failed to create the folder in `queues_dirpath`
    pub fn write_to_mails(
        queues_dirpath: &std::path::Path,
        message_id: &str,
        message: &MessageBody,
    ) -> std::io::Result<()> {
        let buf = std::path::PathBuf::from(queues_dirpath).join("mails");
        if !buf.exists() {
            std::fs::DirBuilder::new().recursive(true).create(&buf)?;
        }
        let mut to_write = buf.join(message_id);
        to_write.set_extension(match &message {
            MessageBody::Raw(_) => "eml",
            MessageBody::Parsed(_) => "json",
        });

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&to_write)?;

        std::io::Write::write_all(
            &mut file,
            match message {
                MessageBody::Raw(_) => {
                    format!("{message}")
                }
                MessageBody::Parsed(parsed) => serde_json::to_string(parsed)?,
            }
            .as_bytes(),
        )?;

        Ok(())
    }
}
