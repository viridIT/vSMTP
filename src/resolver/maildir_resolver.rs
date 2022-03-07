/**
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
**/
use crate::{
    config::{log_channel::RESOLVER, server_config::ServerConfig},
    libc_abstraction::chown_file,
    rules::address::Address,
    smtp::mail::{Body, MailContext, MessageMetadata},
};

use super::Resolver;

/// see https://en.wikipedia.org/wiki/Maildir
#[derive(Default)]
pub struct MailDirResolver;

impl MailDirResolver {
    // getting user's home directory using getpwuid.
    fn get_maildir_path(user: &users::User) -> anyhow::Result<std::path::PathBuf> {
        let passwd = unsafe { libc::getpwuid(user.uid()) };
        if !passwd.is_null() && !unsafe { *passwd }.pw_dir.is_null() {
            unsafe { std::ffi::CStr::from_ptr((*passwd).pw_dir) }
                .to_str()
                .map(|path| std::path::PathBuf::from_iter([path, "Maildir", "new"]))
                .map_err(|error| {
                    anyhow::anyhow!("unable to get user's home directory: '{}'", error)
                })
        } else {
            anyhow::bail!(std::io::Error::last_os_error())
        }
    }

    /// write to /home/${user}/Maildir/new/ the mail body sent by the client.
    /// the format of the file name is the following:
    /// `{timestamp}.{content size}{deliveries}{rcpt id}.vsmtp`
    fn write_to_maildir(
        rcpt: &Address,
        metadata: &MessageMetadata,
        content: &str,
    ) -> anyhow::Result<()> {
        // FIXME: use UsersCache.
        match users::get_user_by_name(rcpt.local_part()) {
            Some(user) => {
                let mut maildir = Self::get_maildir_path(&user)?;

                // create and set rights for the MailDir folder if it doesn't exists.
                if !maildir.exists() {
                    std::fs::create_dir_all(&maildir)?;
                    chown_file(&maildir, &user)?;
                    chown_file(
                        maildir
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("Maildir parent folder is missing."))?,
                        &user,
                    )?;
                }

                // NOTE: see https://en.wikipedia.org/wiki/Maildir
                maildir.push(format!("{}.vsmtp", metadata.message_id));

                // TODO: should loop if an email name is conflicting with another.
                let mut email = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&maildir)?;

                std::io::Write::write_all(&mut email, content.as_bytes())?;

                chown_file(&maildir, &user)?;
            }
            None => anyhow::bail!("unable to get user '{}' by name", rcpt.local_part()),
        }

        log::debug!(
            target: RESOLVER,
            "{} bytes written to {}'s mail spool",
            content.len(),
            rcpt
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl Resolver for MailDirResolver {
    async fn deliver(&mut self, _: &ServerConfig, mail: &MailContext) -> anyhow::Result<()> {
        // NOTE: see https://docs.rs/tempfile/3.0.7/tempfile/index.html
        //       and https://en.wikipedia.org/wiki/Maildir

        log::trace!(target: RESOLVER, "mail: {:#?}", mail.envelop);

        let not_local_users = mail
            .envelop
            .rcpt
            .iter()
            .filter(|i| users::get_user_by_name(i.local_part()).is_some())
            .collect::<Vec<_>>();

        if !not_local_users.is_empty() {
            log::trace!(
                target: RESOLVER,
                "Users '{:?}' not found on the system, skipping delivery ...",
                not_local_users
            );
            anyhow::bail!(
                "Users '{:?}' not found on the system, skipping delivery ...",
                not_local_users
            )
        } else {
            for rcpt in &mail.envelop.rcpt {
                log::debug!(target: RESOLVER, "writing email to {}'s inbox.", rcpt);

                match &mail.body {
                    Body::Empty => {
                        anyhow::bail!("failed to write email using maildir: body is empty")
                    }
                    Body::Raw(content) => {
                        Self::write_to_maildir(rcpt, mail.metadata.as_ref().unwrap(), content)
                            .map_err(|error| {
                                log::error!(
                                    target: RESOLVER,
                                    "Couldn't write email to inbox: {:?}",
                                    error
                                );
                                error
                            })?
                    }
                    Body::Parsed(email) => {
                        let (headers, body) = email.to_raw();

                        Self::write_to_maildir(
                            rcpt,
                            mail.metadata.as_ref().unwrap(),
                            format!("{headers}\n{body}").as_str(),
                        )
                        .map_err(|error| {
                            log::error!(
                                target: RESOLVER,
                                "Couldn't write email to inbox: {:?}",
                                error
                            );
                            error
                        })?
                    }
                }
            }
            Ok(())
        }
    }
}
