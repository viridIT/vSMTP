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

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelop {
    pub helo: String,
    pub mail_from: String,
    pub recipients: Vec<String>,
    // TODO: remove from envelop, format!("{connection_timestamp}_{pid/thread}_{mail_id}_{rcpt_id}")
    pub msg_id: String,
}

impl Envelop {
    // TODO: need error handling (i.e. @blablah.com should return an error.)
    pub fn get_rcpt_usernames(&self) -> Vec<&str> {
        self.recipients
            .iter()
            .map(|recipient| {
                // TODO: find a way to remove everything after the '@' delimiter.
                // for now just using splitn, not really clean.
                Some(recipient.split_once("@").map_or(&**recipient, |x| x.0)).unwrap_or(recipient)
            })
            .collect()
    }
}
