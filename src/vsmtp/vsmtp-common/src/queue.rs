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
use crate::{
    log_channels,
    mail_context::{MailContext, MessageBody},
};
use anyhow::Context;

/// identifiers for all mail queues.
#[derive(Debug, PartialEq, Copy, Clone, strum::Display, strum::EnumString, strum::EnumIter)]
#[strum(serialize_all = "lowercase")]
pub enum Queue {
    /// Postq.
    Working,
    /// 1st attempt to deliver.
    Deliver,
    /// the message has been delegated.
    Delegated,
    /// 1st delivery attempt failed.
    Deferred,
    /// Too many attempts failed.
    Dead,
}

/// Syntax sugar for access of queues folder and queues items
///
/// # Errors
///
/// * if `create_if_missing` is provided, will attempt to create the folder
#[allow(clippy::module_name_repetitions)]
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

        log::debug!(
            target: log_channels::QUEUE,
            "mail {message_id} successfully written to {self} queue"
        );

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
            MessageBody::Raw { .. } => "eml",
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
                MessageBody::Raw { .. } => message.to_string(),
                MessageBody::Parsed(parsed) => serde_json::to_string(parsed)?,
            }
            .as_bytes(),
        )?;

        Ok(())
    }

    ///
    /// # Errors
    pub async fn read_mail_context(
        &self,
        dirpath: &std::path::Path,
        id: &str,
    ) -> anyhow::Result<MailContext> {
        let context_filepath = queue_path!(&dirpath, self, &id);

        let content = tokio::fs::read_to_string(&context_filepath)
            .await
            .with_context(|| format!("Cannot read file '{}'", context_filepath.display()))?;

        serde_json::from_str::<MailContext>(&content)
            .with_context(|| format!("Cannot deserialize: '{content:?}'"))
    }

    /// # Errors
    pub async fn read_mail_message(
        dirpath: &std::path::Path,
        id: &str,
    ) -> anyhow::Result<MessageBody> {
        let mut message_filepath =
            std::path::PathBuf::from_iter([dirpath.to_path_buf(), "mails".into(), id.into()]);

        message_filepath.set_extension("json");
        if message_filepath.exists() {
            let content = tokio::fs::read_to_string(&message_filepath)
                .await
                .with_context(|| format!("Cannot read file '{}'", message_filepath.display()))?;

            return serde_json::from_str::<MessageBody>(&content)
                .with_context(|| format!("Cannot deserialize: '{content:?}'"));
        }

        message_filepath.set_extension("eml");
        if message_filepath.exists() {
            let content = tokio::fs::read_to_string(&message_filepath)
                .await
                .with_context(|| format!("Cannot read file '{}'", message_filepath.display()))?;

            let (headers, body) = content
                .split_once("\r\n\r\n")
                .ok_or_else(|| anyhow::anyhow!("Cannot find message body"))?;

            return Ok(MessageBody::Raw {
                headers: headers.lines().map(str::to_string).collect(),
                body: Some(body.to_string()),
            });
        }
        anyhow::bail!("failed does not exist")
    }

    /// Return a message body from a file path.
    /// Try to parse the file as JSON, if it fails, try to parse it as plain text.
    ///
    /// # Errors
    ///
    /// * file(s) not found
    /// * file found but failed to read
    /// * file read but failed to serialize
    pub async fn read(
        &self,
        dirpath: &std::path::Path,
        id: &str,
    ) -> anyhow::Result<(MailContext, MessageBody)> {
        let (context, message) = tokio::join!(
            self.read_mail_context(dirpath, id),
            Self::read_mail_message(dirpath, id)
        );

        Ok((context?, message?))
    }

    /// Remove a message from the queue system.
    ///
    /// # Errors
    ///
    /// * see [`std::fs::remove_file`]
    pub fn remove(&self, dirpath: &std::path::Path, id: &str) -> anyhow::Result<()> {
        std::fs::remove_file(queue_path!(&dirpath, self, &id))
            .with_context(|| format!("failed to remove `{id}` from the `{self}` queue"))
    }

    /// Write the `ctx` to `other` **AND THEN** remove `ctx` from `self`
    ///
    /// # Errors
    ///
    /// * see [`Queue::write_to_queue`]
    /// * see [`Queue::remove`]
    pub fn move_to(
        &self,
        other: &Self,
        queues_dirpath: &std::path::Path,
        ctx: &MailContext,
    ) -> anyhow::Result<()> {
        self.remove(
            queues_dirpath,
            &ctx.metadata
                .as_ref()
                .expect("message is ill-formed")
                .message_id,
        )?;

        other.write_to_queue(queues_dirpath, ctx)?;

        Ok(())
    }
}
