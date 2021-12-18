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

pub struct ResolverWriteDisk;
impl ResolverWriteDisk {
    pub fn init_spool_folder(path: &str) -> Result<std::path::PathBuf, std::io::Error> {
        let filepath = <std::path::PathBuf as std::str::FromStr>::from_str(path)
            .expect("Failed to initialize the spool folder");
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
    /// `{timestamp}.Size={content size}.D={deliveries}.ID={unique_id}.vsmtp`
    fn write_to_maildir(
        rcpt: &Address,
        timestamp: &str,
        unique_id: usize,
        content: &str,
    ) -> std::io::Result<()> {
        lazy_static::lazy_static! {
            static ref DELIVERIES: std::sync::Mutex<Wrapper> = std::sync::Mutex::new(Wrapper{0:0});
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

        let mut folder =
            std::path::PathBuf::from_iter(["/", "home", rcpt.local_part(), "Maildir", "new"]);
        std::fs::create_dir_all(&folder)?;

        // TODO: follow maildir's writing convention.
        // NOTE: see https://en.wikipedia.org/wiki/Maildir
        let filename = format!(
            "{}.{}{}{}.vsmtp",
            timestamp,
            content.as_bytes().len(),
            delivery_count.0,
            unique_id
        );
        folder.push(filename);

        let mut inbox = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&folder)?;

        std::io::Write::write_all(&mut inbox, content.as_bytes())?;

        delivery_count.0 += 1;

        // we can unwrap here since we've already checked if the user existed.
        let uid = crate::rules::rule_engine::get_user_by_name(rcpt.local_part())
            .unwrap()
            .uid();

        if unsafe {
            libc::chown(
                // NOTE: to_string_lossy().as_bytes() isn't the right way of converting a PathBuf
                //       to a CString because it is plateform independant.
                std::ffi::CString::new(folder.to_string_lossy().as_bytes())?.as_ptr(),
                uid,
                uid,
            )
        } != 0
        {
            log::error!("unable to setuid of user {}", rcpt.local_part());
            return Err(std::io::Error::last_os_error());
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
        mail: &crate::model::mail::MailContext,
    ) -> std::io::Result<()> {
        let folder = format!("{}/to_process", spool_dir);
        std::fs::create_dir_all(&folder)?;

        let mut to_process = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(format!(
                "{}/{}_{:?}.json",
                folder,
                mail.timestamp
                    .unwrap()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                std::thread::current().id()
            ))?;

        std::io::Write::write_all(&mut to_process, serde_json::to_string(&mail)?.as_bytes())
    }
}

#[async_trait::async_trait]
impl DataEndResolver for ResolverWriteDisk {
    async fn on_data_end(
        _config: &ServerConfig,
        mail: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        // TODO: use temporary file unix syscall to generate temporary files
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
                    &match mail.timestamp {
                        Some(timestamp) => match timestamp.elapsed() {
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
