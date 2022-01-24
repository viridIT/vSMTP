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
    config::server_config::ServerConfig, model::mail::MailContext, smtp::code::SMTPReplyCode,
};

pub mod maildir_resolver;
pub mod smtp_resolver;

#[async_trait::async_trait]
pub trait DataEndResolver {
    async fn on_data_end(
        &mut self,
        config: &ServerConfig,
        mail: &MailContext,
    ) -> std::io::Result<SMTPReplyCode>;
}
