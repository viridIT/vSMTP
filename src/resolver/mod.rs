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
use crate::{config::server_config::ServerConfig, model::mail::MailContext};

pub mod maildir_resolver;
pub mod mbox_resolver;
pub mod smtp_resolver;

#[async_trait::async_trait]
pub trait Resolver {
    async fn deliver(&mut self, config: &ServerConfig, mail: &MailContext) -> anyhow::Result<()>;
}

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
