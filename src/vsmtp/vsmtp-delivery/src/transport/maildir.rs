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
use super::Transport;
use crate::transport::log_channels;
use anyhow::Context;
use vsmtp_common::{
    libc_abstraction::{chown, getpwuid},
    mail_context::MessageMetadata,
    rcpt::Rcpt,
    re::{anyhow, log},
    transfer::{EmailTransferStatus, TransferErrors},
};
use vsmtp_config::{re::users, Config};

/// see <https://en.wikipedia.org/wiki/Maildir>
//
// NOTE: see https://docs.rs/tempfile/3.0.7/tempfile/index.html
//       and https://en.wikipedia.org/wiki/Maildir
#[derive(Default)]
pub struct Maildir;

#[async_trait::async_trait]
impl Transport for Maildir {
    async fn deliver(
        self,
        config: &Config,
        metadata: &MessageMetadata,
        _: &vsmtp_common::Address,
        mut to: Vec<Rcpt>,
        content: &str,
    ) -> Vec<Rcpt> {
        for rcpt in &mut to {
            match users::get_user_by_name(rcpt.address.local_part()).map(|user| {
                write_to_maildir(
                    &user,
                    config.server.system.group_local.as_ref(),
                    metadata,
                    content,
                )
            }) {
                Some(Ok(_)) => {
                    log::info!(
                        target: log_channels::MAILDIR,
                        "(msg={}) successfully delivered to {rcpt} as maildir",
                        metadata.message_id
                    );

                    rcpt.email_status = EmailTransferStatus::Sent {
                        timestamp: std::time::SystemTime::now(),
                    }
                }
                Some(Err(e)) => {
                    log::error!(
                        target: log_channels::MAILDIR,
                        "(msg={}) failed to write email in maildir of '{rcpt}': {e}",
                        metadata.message_id
                    );

                    rcpt.email_status.held_back(e);
                }
                None => {
                    log::error!(
                        target: log_channels::MAILDIR,
                        "(msg={}) failed to write email in maildir of '{}': '{}' is not a user",
                        metadata.message_id,
                        rcpt.address.local_part(),
                        rcpt.address.local_part()
                    );

                    rcpt.email_status.held_back(TransferErrors::NoSuchMailbox {
                        name: rcpt.address.local_part().to_string(),
                    });
                }
            }
        }
        to
    }
}

// NOTE: see https://en.wikipedia.org/wiki/Maildir
fn create_maildir(
    user: &users::User,
    group_local: Option<&users::Group>,
    metadata: &MessageMetadata,
) -> anyhow::Result<std::path::PathBuf> {
    let mut maildir = std::path::PathBuf::from_iter([getpwuid(user.uid())?, "Maildir".into()]);

    let create_and_chown = |path: &std::path::PathBuf, user: &users::User| -> anyhow::Result<()> {
        if !path.exists() {
            std::fs::create_dir(&path).with_context(|| format!("failed to create {:?}", path))?;
            chown(path, Some(user.uid()), group_local.map(users::Group::gid))
                .with_context(|| format!("failed to set user rights to {:?}", path))?;
        }

        Ok(())
    };

    // create and set rights for the MailDir & new folder if they don't exists.
    create_and_chown(&maildir, user)?;
    maildir.push("new");
    create_and_chown(&maildir, user)?;
    maildir.push(format!("{}.eml", metadata.message_id));

    Ok(maildir)
}

fn write_to_maildir(
    user: &users::User,
    group_local: Option<&users::Group>,
    metadata: &MessageMetadata,
    content: &str,
) -> anyhow::Result<()> {
    let maildir = create_maildir(user, group_local, metadata)?;

    let mut email = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&maildir)?;

    std::io::Write::write_all(&mut email, content.as_bytes())?;

    chown(
        &maildir,
        Some(user.uid()),
        group_local.map(users::Group::gid),
    )?;

    log::debug!(
        target: log_channels::MAILDIR,
        "(msg={}) {} bytes written to {:?}'s inbox",
        metadata.message_id,
        content.len(),
        user
    );

    Ok(())
}

#[cfg(test)]
mod test {

    use users::os::unix::UserExt;

    use super::*;

    #[test]
    fn test_maildir_path() {
        let user = users::User::new(10000, "test_user", 10001);
        let current = users::get_user_by_uid(users::get_current_uid())
            .expect("current user has been deleted after running this test");

        // NOTE: if a user with uid 10000 exists, this is not guaranteed to fail.
        // maybe iterate over all users beforehand ?
        assert!(getpwuid(user.uid()).is_err());
        assert_eq!(
            getpwuid(current.uid()).unwrap(),
            std::path::Path::new(current.home_dir().as_os_str().to_str().unwrap()),
        );
    }

    #[test]
    #[ignore]
    fn test_writing_to_maildir() {
        let current = users::get_user_by_uid(users::get_current_uid())
            .expect("current user has been deleted after running this test");
        let message_id = "test_message";

        write_to_maildir(
            &current,
            None,
            &MessageMetadata {
                message_id: message_id.to_string(),
                ..MessageMetadata::default()
            },
            "email content",
        )
        .expect("could not write email to maildir");

        let maildir = std::path::PathBuf::from_iter([
            current.home_dir().as_os_str().to_str().unwrap(),
            "Maildir",
            "new",
            &format!("{}.eml", message_id),
        ]);

        assert_eq!(
            "email content".to_string(),
            std::fs::read_to_string(&maildir)
                .unwrap_or_else(|_| panic!("could not read current '{:?}'", maildir))
        );
    }
}
