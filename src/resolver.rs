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
    mailprocessing::mail_receiver::State,
    model::mail::MailContext,
    smtp::code::SMTPReplyCode,
};

#[async_trait::async_trait]
pub trait DataEndResolver {
    async fn on_data_end(config: &ServerConfig, mail: &MailContext) -> (State, SMTPReplyCode);
}

pub struct ResolverWriteDisk;
impl ResolverWriteDisk {
    pub fn init_spool_folder(path: &str) -> Result<std::path::PathBuf, std::io::Error> {
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

    /// write to ${spool_dir}/inbox/${rcpt}
    /// the mail body sent by the client
    fn write_email_to_rcpt_inbox(
        spool_dir: &str,
        rcpt: &str,
        content: &str,
    ) -> std::io::Result<()> {
        let folder = format!("{}/inbox", spool_dir,);
        std::fs::create_dir_all(&folder).unwrap();

        let mut inbox = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            // NOTE: does Path struct path concatenation exists ?
            .open(&format!("{}/{}", folder, rcpt))
            .unwrap();

        std::io::Write::write_all(&mut inbox, content.as_bytes()).unwrap();

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
    async fn on_data_end(config: &ServerConfig, mail: &MailContext) -> (State, SMTPReplyCode) {
        Self::write_mail_to_process(&config.smtp.spool_dir, mail).unwrap();

        log::trace!(target: RESOLVER, "mail: {:#?}", mail.envelop);

        for rcpt in mail.envelop.get_rcpt_usernames() {
            log::debug!(target: RESOLVER, "writing email to {}'s inbox.", rcpt);

            if let Err(e) =
                Self::write_email_to_rcpt_inbox(&config.smtp.spool_dir, rcpt, &mail.body)
            {
                log::error!(target: RESOLVER, "Couldn't write email to inbox: {:?}", e);
            };
        }
        (State::MailFrom, SMTPReplyCode::Code250)
    }
}
