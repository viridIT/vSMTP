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
use crate::smtp::code::SMTPReplyCode;

#[async_trait::async_trait]
pub trait DataEndResolver {
    async fn on_data_end(
        mail: &crate::model::mail::MailContext,
    ) -> (crate::mailprocessing::mail_receiver::State, SMTPReplyCode);
}

#[derive(Debug)]
pub enum SpoolInitializationError {
    ExistAndNotDir(String),
}

impl std::error::Error for SpoolInitializationError {}

impl std::fmt::Display for SpoolInitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpoolInitializationError::ExistAndNotDir(path) => {
                write!(f, "path is not a directory: {}", path)
            }
        }
    }
}

pub struct ResolverWriteDisk;
impl ResolverWriteDisk {
    pub fn init_spool_folder(
        path: &str,
    ) -> Result<std::path::PathBuf, Box<(dyn std::error::Error + 'static)>> {
        let filepath = <std::path::PathBuf as std::str::FromStr>::from_str(path)?;
        if filepath.exists() {
            if filepath.is_dir() {
                log::debug!(target: "mail_receiver", "vmta's mail spool is already initialized.");
                Ok(filepath)
            } else {
                Err(Box::new(SpoolInitializationError::ExistAndNotDir(
                    path.to_string(),
                )))
            }
        } else {
            std::fs::create_dir_all(&filepath)?;
            log::debug!(target: "mail_receiver", "vmta's mail spool initialized.");
            Ok(filepath)
        }
    }

    fn write_email_to_rcpt_inbox(rcpt: &str, content: &str) -> std::io::Result<()> {
        let folder = format!(
            "{}/inbox",
            crate::config::get::<String>("paths.spool_dir")
                .unwrap_or_else(|_| crate::config::DEFAULT_SPOOL_PATH.to_string()),
        );
        std::fs::create_dir_all(&folder).unwrap();

        let mut inbox = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            // NOTE: does Path struct path concatenation exists ?
            .open(&format!("{}/{}", folder, rcpt))
            .unwrap();

        std::io::Write::write_all(&mut inbox, content.as_bytes()).unwrap();

        log::debug!(target: "mail_receiver", "{} bytes written to {}'s mail spool", content.len(), rcpt);

        Ok(())
    }

    fn write_mail_to_process(mail: &crate::model::mail::MailContext) -> std::io::Result<()> {
        let folder = format!(
            "{}/to_process",
            crate::config::get::<String>("paths.spool_dir")
                .unwrap_or_else(|_| crate::config::DEFAULT_SPOOL_PATH.to_string()),
        );
        std::fs::create_dir_all(&folder)?;

        let mut to_process = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(format!("{}/{}.json", folder, mail.envelop.msg_id))?;

        std::io::Write::write_all(&mut to_process, serde_json::to_string(&mail)?.as_bytes())
    }
}

#[async_trait::async_trait]
impl DataEndResolver for ResolverWriteDisk {
    async fn on_data_end(
        mail: &crate::model::mail::MailContext,
    ) -> (crate::mailprocessing::mail_receiver::State, SMTPReplyCode) {
        Self::write_mail_to_process(mail).unwrap();

        log::trace!(target: "mail_receiver", "mail: {:#?}", mail.envelop);

        let content = std::str::from_utf8(&mail.body).unwrap();

        for rcpt in mail.envelop.get_rcpt_usernames() {
            log::debug!(target: "mail_receiver", "writing email to {}'s inbox.", rcpt);

            // TODO: parse each recipient name.
            if let Err(e) = Self::write_email_to_rcpt_inbox(rcpt, content) {
                log::error!(target: "mail_receiver","Couldn't write email to inbox: {:?}", e);
            };
        }
        (
            crate::mailprocessing::mail_receiver::State::MailFrom,
            SMTPReplyCode::Code250,
        )
    }
}
