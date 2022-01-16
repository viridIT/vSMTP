use super::mime_type::Mime;

/// we use Vec instead of a HashMap because header ordering is important.
pub type MailHeaders = Vec<(String, String)>;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
/// see rfc5322 (section 2.1 and 2.3)
pub enum BodyType {
    Regular(Vec<String>),
    Mime(Box<Mime>),
    Undefined,
}

impl ToString for BodyType {
    fn to_string(&self) -> String {
        match self {
            Self::Regular(content) => content.join("\n"),
            Self::Mime(content) => {
                let (headers, body) = content.to_raw();
                format!("{}\n{}", headers, body)
            }
            Self::Undefined => String::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Mail {
    pub headers: MailHeaders,
    pub body: BodyType,
}

impl Mail {
    pub fn to_raw(&self) -> (String, String) {
        (
            self.headers
                .iter()
                .map(|(header, value)| format!("{}: {}", header, value))
                .collect::<Vec<_>>()
                .join("\n"),
            self.body.to_string(),
        )
    }
}
