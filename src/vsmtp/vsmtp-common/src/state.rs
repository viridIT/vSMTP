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
use crate::auth::Mechanism;

/// State of the pipeline SMTP
#[derive(
    Debug,
    Eq,
    PartialEq,
    Hash,
    Clone,
    Ord,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
    strum::EnumString,
    strum::Display,
)]
#[serde(untagged)]
#[allow(clippy::module_name_repetitions)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StateSMTP {
    /// After TCP/IP socket has been accepted
    Connect,
    /// After receiving HELO/EHLO command
    Helo,
    /// After receiving STARTTLS command
    NegotiationTLS,
    /// After receiving AUTH command
    Authenticate(Mechanism, Option<Vec<u8>>),
    /// After receiving MAIL FROM command
    #[strum(serialize = "mail")]
    MailFrom,
    /// After receiving RCPT TO command
    #[strum(serialize = "rcpt")]
    RcptTo,
    /// After receiving DATA command
    Data,
    /// Before write on disk
    PreQ,
    /// After receiving QUIT command
    Stop,
    /// After connection closed
    PostQ,
    /// Right before sending to recipient
    Delivery,
}

impl Default for StateSMTP {
    fn default() -> Self {
        Self::Connect
    }
}
