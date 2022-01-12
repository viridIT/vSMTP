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
    config::{log_channel::RESOLVER, server_config::ServerConfig},
    model::mail::{Body, MailContext, MessageMetadata},
    rules::address::Address,
    smtp::code::SMTPReplyCode,
};

use super::DataEndResolver;

/// sets user & group rights to the given file / folder.
fn chown_file(path: &std::path::Path, user: &users::User) -> std::io::Result<()> {
    if unsafe {
        libc::chown(
            // NOTE: to_string_lossy().as_bytes() isn't the right way of converting a PathBuf
            //       to a CString because it is platform independent.
            std::ffi::CString::new(path.to_string_lossy().as_bytes())?.as_ptr(),
            user.uid(),
            user.uid(),
        )
    } != 0
    {
        log::error!("unable to setuid of user {:?}", user.name());
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[derive(Default)]
pub struct MailDirResolver;

impl MailDirResolver {
    // getting user's home directory using getpwuid.
    unsafe fn get_maildir_path(
        user: &std::sync::Arc<users::User>,
    ) -> std::io::Result<std::path::PathBuf> {
        let passwd = libc::getpwuid(user.uid());
        if !passwd.is_null() && !(*passwd).pw_dir.is_null() {
            match std::ffi::CStr::from_ptr((*passwd).pw_dir).to_str() {
                Ok(path) => Ok(std::path::PathBuf::from_iter([
                    &path.to_string(),
                    "Maildir",
                    "new",
                ])),
                Err(error) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("unable to get user's home directory: {}", error),
                    ))
                }
            }
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    /// write to /home/${user}/Maildir/ the mail body sent by the client.
    /// the format of the file name is the following:
    /// `{timestamp}.{content size}{deliveries}{rcpt id}.vsmtp`
    fn write_to_maildir(
        rcpt: &Address,
        metadata: &MessageMetadata,
        content: &str,
    ) -> std::io::Result<()> {
        match crate::rules::rule_engine::get_user_by_name(rcpt.local_part()) {
            Some(user) => {
                let mut maildir = unsafe { Self::get_maildir_path(&user)? };

                // create and set rights for the MailDir folder if it doesn't exists.
                if !maildir.exists() {
                    std::fs::create_dir_all(&maildir)?;
                    chown_file(&maildir, &user)?;
                    chown_file(
                        maildir.parent().ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Maildir parent folder is missing.",
                            )
                        })?,
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
            None => {
                log::error!("unable to get user '{}' by name", rcpt.local_part());
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "unable to get user",
                ));
            }
        }

        log::debug!(
            target: RESOLVER,
            "{} bytes written to {}'s mail spool",
            content.len(),
            rcpt
        );

        Ok(())
    }

    /// write to ${spool_dir}/to_process/${timestamp}_${thread_id}.json
    /// the mail context in a serialized json format
    /// NOTE: unused for now, as the delivery system isn't ready yet.
    #[allow(unused)]
    fn write_mail_to_process(
        spool_dir: &str,
        ctx: &crate::model::mail::MailContext,
    ) -> std::io::Result<()> {
        let folder = format!("{}/to_process", spool_dir);
        std::fs::create_dir_all(&folder)?;

        let mut to_process = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(format!(
                "{}/{}.json",
                folder,
                ctx.metadata.as_ref().unwrap().message_id,
            ))?;

        std::io::Write::write_all(&mut to_process, serde_json::to_string(&ctx)?.as_bytes())
    }
}

#[async_trait::async_trait]
impl DataEndResolver for MailDirResolver {
    async fn on_data_end(
        &mut self,
        _config: &ServerConfig,
        mail: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        // NOTE: see https://docs.rs/tempfile/3.0.7/tempfile/index.html
        //       and https://en.wikipedia.org/wiki/Maildir
        //
        // Self::write_mail_to_process(&config.smtp.spool_dir, mail)

        log::trace!(target: RESOLVER, "mail: {:#?}", mail.envelop);

        let content = match &mail.body {
            Body::Raw(content) => content.clone(),
            Body::Parsed(content) => {
                let (headers, body) = content.to_raw();
                [headers, body].join("\n\n")
            }
        };

        for rcpt in mail.envelop.rcpt.iter() {
            if crate::rules::rule_engine::user_exists(rcpt.local_part()) {
                log::debug!(target: RESOLVER, "writing email to {}'s inbox.", rcpt);

                if let Err(error) =
                    Self::write_to_maildir(rcpt, mail.metadata.as_ref().unwrap(), &content)
                {
                    log::error!(
                        target: RESOLVER,
                        "Couldn't write email to inbox: {:?}",
                        error
                    );

                    return Err(error);
                }
            } else {
                log::trace!(
                    target: RESOLVER,
                    "User {} not found on the system, skipping delivery ...",
                    rcpt
                );
            }
        }
        Ok(SMTPReplyCode::Code250)
    }
}
