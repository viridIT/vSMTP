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
use crate::{envelop::Envelop, mail::Mail, status::Status};

// use crate::{mime::mail::Mail, rules::rule_engine::Status};

pub const MAIL_CAPACITY: usize = 10_000_000; // 10MB

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MessageMetadata {
    /// instant when the last "MAIL FROM" has been received.
    pub timestamp: std::time::SystemTime,
    /// unique id generated when the "MAIL FROM" has been received.
    /// format: {mail timestamp}{connection timestamp}{process id}
    pub message_id: String,
    /// number of times the mta tried to send the email.
    pub retry: usize,
    /// the resolver chosen to deliver the message.
    pub resolver: String,
    /// whether further rule analysis has been skipped.
    pub skipped: Option<Status>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            message_id: String::default(),
            retry: Default::default(),
            resolver: "default".to_string(),
            skipped: None,
        }
    }
}

/// Message body issued by a SMTP transaction
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Body {
    /// Nothing
    Empty,
    /// The raw representation of the message
    Raw(String),
    /// The message parsed using [MailMimeParser]
    Parsed(Box<Mail>),
}

/// Representation of one mail obtained by a transaction SMTP
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MailContext {
    /// time of connexion by the client.
    pub connexion_timestamp: std::time::SystemTime,
    /// emitter of the mail
    pub client_addr: std::net::SocketAddr,
    /// envelop of the message
    pub envelop: Envelop,
    /// content of the message
    pub body: Body,
    /// metadata
    pub metadata: Option<MessageMetadata>,
}
