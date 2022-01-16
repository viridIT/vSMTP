use super::{
    error::{ParserError, ParserResult},
    mail::Mail,
};

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MimeHeader {
    pub name: String,
    pub value: String,
    /// parameter ordering does not matter.
    pub args: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum MimeBodyType {
    Regular(Vec<String>),
    Multipart(MimeMultipart),
    Embedded(Mail),
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
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

impl ToString for MimeHeader {
    /// ```
    /// use vsmtp::mime::mime_type::MimeHeader;
    ///
    /// let input = MimeHeader {
    ///     name: "Content-Type".to_string(),
    ///     value: "text/plain".to_string(),
    ///     args: std::collections::HashMap::from([
    ///       ("charset".to_string(), "us-ascii".to_string()),
    ///       ("another".to_string(), "argument".to_string()),
    ///    ]),
    /// };
    ///
    /// assert!(
    ///     // arguments can be in any order.
    ///     input.to_string() == "Content-Type: text/plain; charset=us-ascii; another=argument".to_string() ||
    ///     input.to_string() == "Content-Type: text/plain; another=argument; charset=us-ascii".to_string()
    /// );
    ///
    /// let input = MimeHeader {
    ///     name: "Content-Type".to_string(),
    ///     value: "application/foobar".to_string(),
    ///     args: std::collections::HashMap::default(),
    /// };
    ///
    /// assert_eq!(input.to_string(),
    ///   "Content-Type: application/foobar".to_string()
    /// );
    /// ```
    fn to_string(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join("; ");

        format!(
            "{}: {}{}",
            self.name,
            self.value,
            if args.is_empty() {
                String::default()
            } else {
                format!("; {}", args)
            }
        )
    }
}

// impl ToString for MimeBodyType {
//     /// ```
//     /// use vsmtp::mime::mime_type::MimeBodyType;
//     /// use vsmtp::mime::mime_type::MimeMultipart;
//     ///
//     /// let body = MimeBodyType::Regular(
//     ///     vec![
//     ///         "The two analysts that I have had contact with are Matt Lenhart  and Vishal".to_string(),
//     ///         "Apte.".to_string(),
//     ///         "Matt will be represented by Jeff Shankman.".to_string(),
//     ///         "Vishal joined our group in October.  He was in the Power Trading Group for".to_string(),
//     ///         "the first 9 months.".to_string(),
//     ///     ],
//     /// );
//     ///
//     /// assert_eq!(body.to_string(),
//     /// r#"The two analysts that I have had contact with are Matt Lenhart  and Vishal
//     ///Apte.
//     ///Matt will be represented by Jeff Shankman.
//     ///Vishal joined our group in October.  He was in the Power Trading Group for
//     ///the first 9 months."#);
//     /// ```
//     fn to_string(&self) -> String {
//         match self {
//             MimeBodyType::Regular(regular) => regular.join("\n"),
//             MimeBodyType::Multipart(multipart) => multipart.to_string(),
//             MimeBodyType::Embedded(mail) => {
//                 let (headers, body) = mail.to_raw();
//                 format!("{}\n{}", headers, body)
//             }
//         }
//     }
// }

impl MimeMultipart {
    fn to_raw(&self, boundary: &str) -> String {
        format!(
            //
            //  preamble
            //  --boundary
            //  *{ headers \n body \n boundary}
            //  epilogue || nothing
            //  --end-boundary--
            "\n{}\n--{}\n{}\n{}--{}--\n",
            self.preamble,
            boundary,
            self.parts
                .iter()
                .map(Mime::to_raw)
                .map(|(headers, body)| format!("{}\n{}", headers, body))
                .collect::<Vec<_>>()
                .join(&format!("\n--{}\n", boundary)),
            if self.epilogue.is_empty() {
                "".to_string()
            } else {
                format!("{}\n", self.epilogue)
            },
            boundary,
        )
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Mime {
    pub headers: Vec<MimeHeader>,
    pub content: MimeBodyType,
}

impl Mime {
    pub fn to_raw(&self) -> (String, String) {
        (
            self.headers
                .iter()
                .map(MimeHeader::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
            match &self.content {
                MimeBodyType::Regular(regular) => regular.join("\n"),
                MimeBodyType::Multipart(multipart) => {
                    let boundary = self
                        .headers
                        .iter()
                        .find(|header| header.args.get("boundary").is_some());
                    multipart.to_raw(
                        boundary
                            .expect("multipart mime message is missing it's boundary argument.")
                            .args
                            .get("boundary")
                            .unwrap(),
                    )
                }
                MimeBodyType::Embedded(mail) => {
                    let (headers, body) = mail.to_raw();
                    format!("{}\n{}", headers, body)
                }
            },
        )
    }
}
