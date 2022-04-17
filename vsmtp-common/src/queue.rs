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
use crate::mail_context::MailContext;
use anyhow::Context;

/// identifiers for all mail queues.
#[derive(Debug, PartialEq, Copy, Clone, strum::EnumIter)]
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

impl std::fmt::Display for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Working => "working",
            Self::Deliver => "deliver",
            Self::Deferred => "deferred",
            Self::Dead => "dead",
        })
    }
}

impl std::str::FromStr for Queue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "working" => Ok(Self::Working),
            "deliver" => Ok(Self::Deliver),
            "deferred" => Ok(Self::Deferred),
            "dead" => Ok(Self::Dead),
            _ => anyhow::bail!("not a valid queue: '{}'", s),
        }
    }
}

impl Queue {
    /// Return a valid path of the Queue, depending of @queues_dirpath,
    /// create the folder and the parents if not existing
    ///
    /// # Errors
    ///
    /// * failed to create the folders
    pub fn to_path(
        &self,
        queues_dirpath: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<std::path::PathBuf> {
        let dir = queues_dirpath.into().join(format!("{}", self));
        if !dir.exists() {
            std::fs::DirBuilder::new().recursive(true).create(&dir)?;
        }
        Ok(dir)
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

        let to_deliver = self.to_path(&queues_dirpath)?.join(message_id);

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Queue;

    #[test]
    fn error() {
        assert_eq!(
            format!("{}", Queue::from_str("foobar").unwrap_err()),
            "not a valid queue: 'foobar'"
        );
    }

    #[test]
    fn same() {
        for s in <Queue as strum::IntoEnumIterator>::iter() {
            println!("{:?}", s);
            assert_eq!(Queue::from_str(&format!("{}", s)).unwrap(), s);
        }
    }
}
