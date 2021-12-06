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

pub struct MailContext {
    pub envelop: super::envelop::Envelop,
    pub body: Vec<u8>,
}

impl MailContext {
    pub(crate) fn generate_message_id(&mut self) {
        self.envelop.msg_id = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
    }
}

impl serde::Serialize for MailContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct(
            "MailContext",
            self.envelop.helo.len()
                + self.envelop.mail_from.len()
                + self.envelop.recipients.iter().fold(0, |s, i| s + i.len())
                + self.body.len(),
        )?;
        serde::ser::SerializeStruct::serialize_field(&mut state, "envelop", &self.envelop)?;
        serde::ser::SerializeStruct::serialize_field(
            &mut state,
            "body",
            std::str::from_utf8(&self.body).unwrap(),
        )?;
        serde::ser::SerializeStruct::end(state)
    }
}

impl<'de> serde::Deserialize<'de> for MailContext {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        todo!()
    }
}
