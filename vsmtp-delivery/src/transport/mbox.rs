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
use super::Transport;

use anyhow::Context;
use vsmtp_common::{
    libc_abstraction::chown_file, mail_context::MessageMetadata, rcpt::Rcpt,
    transfer::EmailTransferStatus,
};
use vsmtp_config::{log_channel::DELIVER, Config};

const CTIME_FORMAT: &[time::format_description::FormatItem<'_>] = time::macros::format_description!(
    "[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]:[second] [year]"
);

#[derive(Default)]
/// resolver use to write emails on the system following the
/// application/mbox Media Type.
/// (see [rfc4155](https://datatracker.ietf.org/doc/html/rfc4155#appendix-A))
pub struct MBox;

#[async_trait::async_trait]
impl Transport for MBox {
    async fn deliver(
        &mut self,
        _: &Config,
        metadata: &MessageMetadata,
        from: &vsmtp_common::address::Address,
        to: &mut [Rcpt],
        content: &str,
    ) -> anyhow::Result<()> {
        let timestamp = get_mbox_timestamp_format(metadata);
        let content = build_mbox_message(from, &timestamp, content);

        // FIXME: use UsersCache.
        for rcpt in to.iter_mut() {
            if let Some(user) = users::get_user_by_name(rcpt.address.local_part()) {
                // NOTE: only linux system is supported here, is the
                //       path to all mboxes always /var/mail ?
                if let Err(err) = write_content_to_mbox(
                    &std::path::PathBuf::from_iter(["/", "var", "mail", rcpt.address.local_part()]),
                    &user,
                    &content,
                ) {
                    log::error!(
                        target: DELIVER,
                        "failed to write email '{}' in mbox of '{rcpt}': {err}",
                        metadata.message_id
                    );

                    rcpt.email_status = match rcpt.email_status {
                        EmailTransferStatus::HeldBack(count) => {
                            EmailTransferStatus::HeldBack(count)
                        }
                        _ => EmailTransferStatus::HeldBack(0),
                    };
                } else {
                    rcpt.email_status = EmailTransferStatus::Sent;
                }
            } else {
                log::error!(
                    target: DELIVER,
                    "failed to write email '{}' in mbox of '{rcpt}': '{rcpt}' is not a user",
                    metadata.message_id
                );

                rcpt.email_status = match rcpt.email_status {
                    EmailTransferStatus::HeldBack(count) => EmailTransferStatus::HeldBack(count),
                    _ => EmailTransferStatus::HeldBack(0),
                };
            };
        }

        Ok(())
    }
}

fn get_mbox_timestamp_format(metadata: &MessageMetadata) -> String {
    let odt: time::OffsetDateTime = metadata.timestamp.into();

    odt.format(&CTIME_FORMAT)
        .unwrap_or_else(|_| String::default())
}

fn build_mbox_message(
    from: &vsmtp_common::address::Address,
    timestamp: &str,
    content: &str,
) -> std::string::String {
    format!("From {} {}\n{}\n", from, timestamp, content)
}

fn write_content_to_mbox(
    mbox: &std::path::Path,
    user: &users::User,
    content: &str,
) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&mbox)
        .with_context(|| format!("could not open {:?} mbox", mbox))?;

    chown_file(mbox, user).with_context(|| format!("could not set owner for '{:?}' mbox", mbox))?;

    std::io::Write::write_all(&mut file, content.as_bytes())
        .with_context(|| format!("could not write email to '{:?}' mbox", mbox))?;

    log::debug!(
        target: DELIVER,
        "{} bytes written to {:?}",
        content.len(),
        mbox
    );

    Ok(())
}

#[cfg(test)]
mod test {

    use vsmtp_common::address::Address;

    use super::*;

    #[test]
    fn test_mbox_time_format() {
        let metadata = MessageMetadata {
            timestamp: std::time::SystemTime::now(),
            ..MessageMetadata::default()
        };

        // FIXME: I did not find a proper way to compare timestamps because the system time
        //        cannot be zero.
        get_mbox_timestamp_format(&metadata);
    }

    #[test]
    fn test_mbox_message_format() {
        let from = Address::try_from("john@doe.com".to_string()).unwrap();
        let content = r#"from: john doe <john@doe.com>
to: green@foo.net
subject: test email

This is a raw email."#;

        let timestamp = get_mbox_timestamp_format(&MessageMetadata {
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            ..MessageMetadata::default()
        });

        let message = build_mbox_message(&from, &timestamp, content);

        assert_eq!(
            r#"From 0 john@doe.com
from: john doe <john@doe.com>
to: green@foo.net
subject: test email

This is a raw email."#,
            message
        );
    }

    #[test]
    #[ignore]
    fn test_writing_to_mbox() {
        let user = users::get_user_by_uid(users::get_current_uid())
            .expect("current user has been deleted after running this test");
        let content = "From 0 john@doe.com\nfrom: john doe <john@doe.com>\n";
        let mbox =
            std::path::PathBuf::from_iter(["./tests/generated/", user.name().to_str().unwrap()]);

        std::fs::create_dir_all("./tests/generated/").expect("could not create temporary folders");

        write_content_to_mbox(&mbox, &user, content).expect("could not write to mbox");

        assert_eq!(
            content.to_string(),
            std::fs::read_to_string(&mbox).expect("could not read mbox")
        );

        std::fs::remove_file(mbox).unwrap();
    }
}
