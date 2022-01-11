use super::mime_type::Mime;

/// we use Vec instead of a HashMap because header ordering is important.
pub type MailHeaders = Vec<(String, String)>;

#[derive(Debug, PartialEq)]
/// see rfc5322 (section 2.1 and 2.3)
pub enum BodyType {
    Regular(Vec<String>),
    Mime(Box<Mime>),
    Undefined,
}

#[derive(Debug, PartialEq)]
pub struct Mail {
    pub headers: MailHeaders,
    pub body: BodyType,
}
