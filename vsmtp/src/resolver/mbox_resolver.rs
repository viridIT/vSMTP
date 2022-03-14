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
use anyhow::Context;
use vsmtp_common::{
    libc_abstraction::chown_file,
    mail_context::{Body, MailContext, MessageMetadata},
};
use vsmtp_config::{log_channel::DELIVER, ServerConfig};
use vsmtp_server::resolver::Resolver;

const CTIME_FORMAT: &[time::format_description::FormatItem<'_>] = time::macros::format_description!(
    "[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]:[second] [year]"
);

#[derive(Default)]
/// resolver use to write emails on the system following the
/// application/mbox Media Type.
/// (see [rfc4155](https://datatracker.ietf.org/doc/html/rfc4155#appendix-A))
pub struct MBoxResolver;

#[async_trait::async_trait]
impl Resolver for MBoxResolver {
    async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
        let timestamp = get_mbox_timestamp_format(&ctx.metadata);
        let content = build_mbox_message(ctx, &timestamp)?;

        for rcpt in &ctx.envelop.rcpt {
            // FIXME: use UsersCache.
            match users::get_user_by_name(rcpt.address.local_part()) {
                Some(user) => {
                    // NOTE: only linux system is supported here, is the
                    //       path to all mboxes always /var/mail ?
                    write_content_to_mbox(
                        &std::path::PathBuf::from_iter(["/var/mail/", rcpt.address.local_part()]),
                        &user,
                        &content,
                    )?;
                }
                _ => anyhow::bail!("unable to get user '{}' by name", rcpt.address.local_part()),
            }
        }
        Ok(())
    }
}

fn get_mbox_timestamp_format(metadata: &Option<MessageMetadata>) -> String {
    let odt: time::OffsetDateTime = metadata
        .as_ref()
        .map_or_else(std::time::SystemTime::now, |metadata| metadata.timestamp)
        .into();

    odt.format(&CTIME_FORMAT)
        .unwrap_or_else(|_| String::default())
}

fn build_mbox_message(ctx: &MailContext, timestamp: &str) -> anyhow::Result<String> {
    Ok(format!(
        "From {} {}\n{}\n",
        ctx.envelop.mail_from,
        timestamp,
        match &ctx.body {
            Body::Empty => {
                anyhow::bail!("failed to write email using mbox: body is empty")
            }
            Body::Raw(raw) => raw.clone(),
            Body::Parsed(parsed) => parsed.to_raw(),
        }
    ))
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
        .with_context(|| format!("could not open '{:?}' mbox", mbox))?;

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

    use vsmtp_common::{
        address::Address,
        mail::{BodyType, Mail},
    };

    use crate::resolver::get_default_context;

    use super::*;

    #[test]
    fn test_mbox_time_format() {
        let metadata = Some(MessageMetadata {
            timestamp: std::time::SystemTime::now(),
            ..MessageMetadata::default()
        });

        // FIXME: I did not find a proper way to compare timestamps because the system time
        //        cannot be zero.
        get_mbox_timestamp_format(&metadata);
        get_mbox_timestamp_format(&None);
    }

    #[test]
    fn test_mbox_message_empty() {
        let ctx = get_default_context();

        assert!(build_mbox_message(&ctx, &get_mbox_timestamp_format(&ctx.metadata)).is_err());
    }

    #[test]
    fn test_mbox_message_raw_and_parsed() {
        let mut ctx = get_default_context();

        ctx.envelop.mail_from = Address::new("john@doe.com").unwrap();
        ctx.body = Body::Raw(
            r#"from: john doe <john@doe.com>
to: green@foo.net
subject: test email

This is a raw email."#
                .to_string(),
        );

        let timestamp = get_mbox_timestamp_format(&ctx.metadata);

        let message_from_raw = build_mbox_message(&ctx, &timestamp).unwrap();

        ctx.body = Body::Parsed(Box::new(Mail {
            headers: [
                ("from", "john doe <john@doe.com>"),
                ("to", "green@foo.net"),
                ("subject", "test email"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
            body: BodyType::Regular(vec!["This is a raw email.".to_string()]),
        }));

        let message_from_parsed = build_mbox_message(&ctx, &timestamp).unwrap();

        assert_eq!(
            format!(
                "From john@doe.com {}\nfrom: john doe <john@doe.com>\nto: green@foo.net\nsubject: test email\n\nThis is a raw email.\n",
                timestamp
            ),
            message_from_parsed
        );
        assert_eq!(message_from_raw, message_from_parsed);
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
