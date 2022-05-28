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
use crate::rcpt::Rcpt;
use crate::Address;

/// Data receive during a smtp transaction
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Envelop {
    /// result of the HELO/HELO command.
    pub helo: String,
    /// the sender of the email received using the MAIL FROM command.
    pub mail_from: Address,
    /// a list of recipients received using the RCPT TO command.
    pub rcpt: Vec<Rcpt>,
}

impl Default for Envelop {
    fn default() -> Self {
        Self {
            helo: String::default(),
            mail_from: Address::new_unchecked("default@domain.com".to_string()),
            rcpt: vec![],
        }
    }
}
