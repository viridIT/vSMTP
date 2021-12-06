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
    pub mail: String,
    pub rcpt: Vec<String>,
    pub msg_id: String,
}

// TODO: need error handling in case of erroneous formatting.
fn remove_inbox_characters(inbox: &str) -> String {
    inbox.trim().replace(&['<', '>'][..], "")
}

impl Envelop {
    pub fn set_sender(&mut self, sender: &str) {
        self.mail = remove_inbox_characters(sender);
    }

    pub fn add_rcpt(&mut self, recipient: &str) {
        self.rcpt.push(remove_inbox_characters(recipient));
    }

    // TODO: need error handling (i.e. @blablah.com should return an error.)
    pub fn get_rcpt_usernames(&self) -> Vec<&str> {
        self.rcpt
            .iter()
            .map(|recipient| {
                // TODO: find a way to remove everything after the '@' delimiter.
                // for now just using splitn, not really clean.
                Some(recipient.split_once("@").map_or(&**recipient, |x| x.0)).unwrap_or(recipient)
            })
            .collect()
    }
}
