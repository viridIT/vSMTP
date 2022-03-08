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
    smtp::mail::{Body, MailContext},
};

use super::Resolver;

#[derive(Default)]
/// resolver use to write emails on the system following the
/// application/mbox Media Type.
/// (see [rfc4155](https://datatracker.ietf.org/doc/html/rfc4155#appendix-A))
pub struct MBoxResolver;

#[async_trait::async_trait]
impl Resolver for MBoxResolver {
    async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
        for rcpt in &ctx.envelop.rcpt {
            // FIXME: use UsersCache.
            match users::get_user_by_name(rcpt.local_part()) {
                Some(user) => {
                    let timestamp: chrono::DateTime<chrono::offset::Utc> = ctx
                        .metadata
                        .as_ref()
                        .map_or_else(std::time::SystemTime::now, |metadata| metadata.timestamp)
                        .into();
                    let timestamp = timestamp.format("%c");

                    let content = match &ctx.body {
                        Body::Empty => {
                            anyhow::bail!("failed to write email using mbox: body is empty")
                        }
                        Body::Raw(raw) => {
                            format!("From {} {timestamp}\n{raw}\n", ctx.envelop.mail_from)
                        }
                        Body::Parsed(parsed) => {
                            format!(
                                "From {} {}\n{}\n",
                                timestamp,
                                ctx.envelop.mail_from,
                                parsed.to_raw()
                            )
                        }
                    };

                    // NOTE: only linux system is supported here, is the
                    //       path to all mboxes always /var/mail ?
                    let mbox = std::path::PathBuf::from_iter(["/var/mail/", rcpt.local_part()]);
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&mbox)?;
                    chown_file(&mbox, &user)?;

                    std::io::Write::write_all(&mut file, content.as_bytes())?;

                    log::debug!(
                        target: RESOLVER,
                        "{} bytes written to {}'s mbox",
                        content.len(),
                        rcpt
                    );
                }
                _ => anyhow::bail!("unable to get user '{}' by name", rcpt.local_part()),
            }
        }
        Ok(())
    }
}
