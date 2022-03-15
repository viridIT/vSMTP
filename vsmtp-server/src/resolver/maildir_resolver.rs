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
use super::Resolver;

use anyhow::Context;
use vsmtp_common::{
    libc_abstraction::{chown_file, getpwuid},
    mail_context::{Body, MailContext, MessageMetadata},
    rcpt::Rcpt,
};
use vsmtp_config::{log_channel::DELIVER, ServerConfig};

/// see https://en.wikipedia.org/wiki/Maildir
#[derive(Default)]
pub struct MailDirResolver;

#[async_trait::async_trait]
impl Resolver for MailDirResolver {
    // NOTE: see https://docs.rs/tempfile/3.0.7/tempfile/index.html
    //       and https://en.wikipedia.org/wiki/Maildir
    async fn deliver(
        &mut self,
        _: &ServerConfig,
        ctx: &MailContext,
        rcpt: &Rcpt,
    ) -> anyhow::Result<()> {
        let content = match &ctx.body {
            Body::Empty => {
                anyhow::bail!("failed to write email using maildir: body is empty")
            }
            Body::Raw(raw) => raw.clone(),
            Body::Parsed(parsed) => parsed.to_raw(),
        };

        match users::get_user_by_name(rcpt.address.local_part()) {
            Some(user) => {
                if let Err(err) = write_to_maildir(&user, ctx.metadata.as_ref().unwrap(), &content)
                {
                    anyhow::bail!(
                        "could not write email to '{}' maildir directory: {}",
                        rcpt,
                        err
                    )
                }
                Ok(())
            }
            None => anyhow::bail!(
                "could not write email to '{}' maildir directory: user was not found on the system",
                rcpt
            ),
        }
    }
}

// NOTE: see https://en.wikipedia.org/wiki/Maildir
fn create_maildir(
    user: &users::User,
    metadata: &MessageMetadata,
) -> anyhow::Result<std::path::PathBuf> {
    let mut maildir = std::path::PathBuf::from_iter([getpwuid(user.uid())?, "Maildir".into()]);

    let create_and_chown = |path: &std::path::PathBuf, user: &users::User| -> anyhow::Result<()> {
        if !path.exists() {
            std::fs::create_dir(&path).with_context(|| format!("failed to create {:?}", path))?;
            chown_file(path, user)
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
    metadata: &MessageMetadata,
    content: &str,
) -> anyhow::Result<()> {
    let maildir = create_maildir(user, metadata)?;

    let mut email = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&maildir)?;

    std::io::Write::write_all(&mut email, content.as_bytes())?;

    chown_file(&maildir, user)?;

    log::debug!(
        target: DELIVER,
        "{} bytes written to {:?}'s inbox",
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
