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

pub const MAIL_CAPACITY: usize = 10_000_000; // 10MB

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MessageMetadata {
    /// instant when the last "MAIL FROM" has been received.
    pub timestamp: std::time::SystemTime,
    /// unique id generated when the "MAIL FROM" has been received.
    /// format: {mail timestamp}{connection timestamp}{process id (on reboot)}
    pub message_id: String,
    /// number of times the mta tried to send the email.
    pub retry: usize,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            message_id: Default::default(),
            retry: Default::default(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum Body {
    Raw(String),
    Parsed(Box<crate::mime::mail::Mail>),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct MailContext {
    pub envelop: super::envelop::Envelop,
    pub body: Body,
    pub metadata: Option<MessageMetadata>,
}
