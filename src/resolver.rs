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
    config::{log::RESOLVER, server_config::ServerConfig},
    model::mail::MailContext,
    rules::address::Address,
    smtp::code::SMTPReplyCode,
};

#[async_trait::async_trait]
pub trait DataEndResolver {
    async fn on_data_end(
        config: &ServerConfig,
        mail: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error>;
}

// TODO: use a AtomicUsize instead of a wrapper.
struct Wrapper(usize);

pub struct MailDirResolver;
impl MailDirResolver {
    pub fn init_spool_folder(path: &str) -> Result<std::path::PathBuf, std::io::Error> {
        // will never crash, we can unwrap.
        let filepath = <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap();
        if filepath.exists() {
            if filepath.is_dir() {
                log::debug!(
                    target: RESOLVER,
                    "vmta's mail spool is already initialized."
                );
                Ok(filepath)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "path specified is not a folder",
                ))
            }
        } else {
            std::fs::create_dir_all(&filepath)?;
            log::debug!(target: RESOLVER, "vmta's mail spool initialized.");
            Ok(filepath)
        }
    }

    /// write to /home/${user}/Maildir/ the mail body sent by the client.
    /// the format of the file name is the following:
    /// `{timestamp}.{content size}{deliveries}{rcpt id}.vsmtp`
    fn write_to_maildir(
        rcpt: &Address,
        timestamp: &str,
        unique_id: usize,
        content: &str,
    ) -> std::io::Result<()> {
        lazy_static::lazy_static! {
            static ref DELIVERIES: std::sync::Mutex<Wrapper> = std::sync::Mutex::new(Wrapper{0:0});
        }

        match crate::rules::rule_engine::get_user_by_name(rcpt.local_part()) {
            Some(user) => {
                // getting user's home directory using getpwuid.
                let mut maildir = unsafe {
                    let passwd = libc::getpwuid(user.uid());
                    if !passwd.is_null() && !(*passwd).pw_dir.is_null() {
                        match std::ffi::CStr::from_ptr((*passwd).pw_dir).to_str() {
                            Ok(path) => {
                                std::path::PathBuf::from_iter([&path.to_string(), "Maildir", "new"])
                            }
                            Err(error) => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("unable to get user's home directory: {}", error),
                                ))
                            }
                        }
                    } else {
                        return Err(std::io::Error::last_os_error());
                    }
                };

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

                let mut delivery_count = match DELIVERIES.lock() {
                    Ok(count) => count,
                    Err(_) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "delivery mutex poisoned.",
                        ))
                    }
                };

                // NOTE: see https://en.wikipedia.org/wiki/Maildir
                maildir.push(format!(
                    "{}.{}{}{}.vsmtp",
                    timestamp,
                    content.as_bytes().len(),
                    delivery_count.0,
                    unique_id
                ));

                // TODO: should loop if an email name is conflicting with another.
                let mut email = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&maildir)?;

                std::io::Write::write_all(&mut email, content.as_bytes())?;
                delivery_count.0 += 1;

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

/// a simple function that sets user & group rights to the given file / folder.
fn chown_file(path: &std::path::Path, user: &users::User) -> std::io::Result<()> {
    if unsafe {
        libc::chown(
            // NOTE: to_string_lossy().as_bytes() isn't the right way of converting a PathBuf
            //       to a CString because it is plateform independant.
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

#[async_trait::async_trait]
impl DataEndResolver for MailDirResolver {
    async fn on_data_end(
        _config: &ServerConfig,
        mail: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        // NOTE: see https://docs.rs/tempfile/3.0.7/tempfile/index.html
        //       and https://en.wikipedia.org/wiki/Maildir
        //
        // Self::write_mail_to_process(&config.smtp.spool_dir, mail)

        log::trace!(target: RESOLVER, "mail: {:#?}", mail.envelop);

        for (index, rcpt) in mail.envelop.rcpt.iter().enumerate() {
            if crate::rules::rule_engine::user_exists(rcpt.local_part()) {
                log::debug!(target: RESOLVER, "writing email to {}'s inbox.", rcpt);

                if let Err(error) = Self::write_to_maildir(
                    rcpt,
                    &match &mail.metadata {
                        Some(metadata) => match metadata.timestamp.elapsed() {
                            Ok(elapsed) => elapsed.as_nanos().to_string(),
                            Err(error) => {
                                log::error!("failed to deliver mail to '{}': {}", rcpt, error);
                                return Ok(SMTPReplyCode::Code250);
                            }
                        },

                        None => {
                            log::error!("failed to deliver mail to '{}': timestamp for email file name is unavailable", rcpt);
                            return Ok(SMTPReplyCode::Code250);
                        }
                    },
                    index,
                    &mail.body,
                ) {
                    log::error!(
                        target: RESOLVER,
                        "Couldn't write email to inbox: {:?}",
                        error
                    );
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
