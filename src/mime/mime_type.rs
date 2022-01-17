use super::{
    error::{ParserError, ParserResult},
    mail::Mail,
};

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MimeHeader {
    pub name: String,
    pub value: String,
    /// parameter ordering is does not matter.
    pub args: std::collections::HashMap<String, String>,
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum MimeBodyType {
    Regular(Vec<String>),
    Multipart(MimeMultipart),
    Embedded(Mail),
}

#[derive(Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MimeMultipart {
    pub preamble: String,
    pub parts: Vec<Mime>,
    pub epilogue: String,
}

impl MimeHeader {
    /// cut the mime type of the current section and return the type and subtype.
    /// if no content-type header is found, will check the parent for a default
    /// content-type header value.
    ///
    /// see https://datatracker.ietf.org/doc/html/rfc2045#page-14 for default content-type.
    /// see https://datatracker.ietf.org/doc/html/rfc2046#page-26 for digest multipart parent.
    pub fn get_mime_type<'a>(
        headers: &'a [MimeHeader],
        parent: Option<&'a [MimeHeader]>,
    ) -> ParserResult<(&'a str, &'a str)> {
        match headers.iter().find(|h| h.name == "content-type") {
            Some(content_type) => {
                let mut value = content_type.value.splitn(2, '/');

                match (value.next(), value.next()) {
                    (Some(t), Some(subtype)) => Ok((t, subtype)),
                    _ => Err(ParserError::InvalidMail(format!(
                        "Invalid content-type value: {}",
                        content_type.value
                    ))),
                }
            }
            None if parent.is_some() => {
                match parent.unwrap().iter().find(|h| h.name == "content-type") {
                    Some(content_type) if content_type.value == "multipart/digest" => {
                        Ok(("message", "rfc822"))
                    }
                    _ => Ok(("text", "plain")),
                }
            }
            _ => Ok(("text", "plain")),
        }
    }
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Mime {
    pub headers: Vec<MimeHeader>,
    pub content: MimeBodyType,
}
