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
use crate::{auth::Credentials, envelop::Envelop, status::Status};

/// average size of a mail
pub const MAIL_CAPACITY: usize = 10_000_000; // 10MB

/// metadata
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MessageMetadata {
    /// instant when the last "MAIL FROM" has been received.
    pub timestamp: std::time::SystemTime,
    /// unique id generated when the "MAIL FROM" has been received.
    /// format: {mail timestamp}{connection timestamp}{process id}
    pub message_id: String,
    /// whether further rule analysis has been skipped.
    pub skipped: Option<Status>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            message_id: String::default(),
            skipped: None,
        }
    }
}

/// Representation of one connection
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConnectionContext {
    /// time of connection by the client.
    pub timestamp: std::time::SystemTime,
    /// credentials of the client.
    pub credentials: Option<Credentials>,
    /// server's domain of the connection. (from config.server.domain or sni)
    pub server_name: String,
    /// server socket used for this connection.
    pub server_address: std::net::SocketAddr,
    /// is the client authenticated ?
    pub is_authenticated: bool,
    /// is the connection under tls ?
    pub is_secured: bool,
}

/// Representation of one mail obtained by a transaction SMTP
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MailContext {
    /// information of the connection producing this message
    pub connection: ConnectionContext,
    /// emitter of the mail
    pub client_addr: std::net::SocketAddr,
    /// envelop of the message
    pub envelop: Envelop,
    /// metadata
    pub metadata: Option<MessageMetadata>,
}
