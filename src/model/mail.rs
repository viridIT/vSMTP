pub struct MailContext {
    pub envelop: super::envelop::Envelop,
    pub body: Vec<u8>,
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
