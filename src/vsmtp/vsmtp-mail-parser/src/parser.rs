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
use crate::log_channels;

use super::error::{ParserError, ParserResult};
use super::helpers::get_mime_type;
use super::helpers::read_header;
use vsmtp_common::mail_context::MessageBody;
use vsmtp_common::re::anyhow::Context;
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::MailParser;
use vsmtp_common::{BodyType, Mail, MailHeaders};
use vsmtp_common::{Mime, MimeBodyType, MimeHeader, MimeMultipart};

/// BoundaryType
/// a boundary serves as a delimiter between mime parts in a multipart section.
enum BoundaryType {
    Delimiter,
    End,
    OutOfScope,
}

/// Instance parsing a message body
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MailMimeParser {
    boundary_stack: Vec<String>,
}

impl MailParser for MailMimeParser {
    fn parse(&mut self, data: Vec<String>) -> anyhow::Result<MessageBody> {
        let data = data.iter().map(String::as_str).collect::<Vec<_>>();
        Ok(MessageBody::Parsed(Box::new(
            self.parse_inner(&mut &data[..]).context("parsing failed")?,
        )))
    }
}

impl MailMimeParser {
    fn parse_inner(&mut self, content: &mut &[&str]) -> ParserResult<Mail> {
        let mut headers = MailHeaders::with_capacity(10);
        let mut mime_headers = Vec::with_capacity(10);

        while !content.is_empty() {
            match read_header(content) {
                Some((name, value)) if is_mime_header(&name) => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "new mime header found: '{}' => '{}'",
                        name,
                        value
                    );
                    mime_headers.push(get_mime_header(&name, &value));
                }

                Some((name, value)) => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "new header found: '{}' => '{}'",
                        name,
                        value
                    );
                    headers.push((name, value));
                }

                None => {
                    // there is an empty lines after headers
                    *content = &content[1..];

                    log::debug!(
                        target: log_channels::PARSER,
                        "finished parsing headers, body found."
                    );

                    check_mandatory_headers(&headers)?;
                    let has_mime_version = headers.iter().any(|(name, _)| name == "mime-version");

                    log::debug!(
                        target: log_channels::PARSER,
                        "mime-version header found?: {}",
                        has_mime_version
                    );

                    return Ok(Mail {
                        headers,
                        body: if has_mime_version {
                            BodyType::Mime(Box::new(self.as_mime_body(
                                content,
                                mime_headers,
                                None,
                            )?))
                        } else {
                            BodyType::Regular(self.as_regular_body(content)?)
                        },
                    });
                }
            };

            *content = &content[1..];
        }

        Ok(Mail {
            headers,
            body: BodyType::Undefined,
        })
    }

    fn check_boundary(&self, line: &str) -> Option<BoundaryType> {
        // we start by checking if the stack as any boundary.
        match self.boundary_stack.last() {
            Some(b) => match get_boundary_type(line, b) {
                None => {
                    // else we need to check the entire stack (except for the last element)
                    // in case of a bad formatted multipart message.
                    if self.boundary_stack[..self.boundary_stack.len() - 1]
                        .iter()
                        .any(|b| get_boundary_type(line, b).is_some())
                    {
                        Some(BoundaryType::OutOfScope)
                    } else {
                        None
                    }
                }
                // if the current scoped boundary is detected, we can return it's type.
                Some(t) => Some(t),
            },
            // if their are no boundaries to check, we just return none.
            _ => None,
        }
    }

    fn as_regular_body(&self, content: &mut &[&str]) -> ParserResult<Vec<String>> {
        let mut body = Vec::with_capacity(100);
        log::debug!(
            target: log_channels::PARSER,
            "storing body of regular message."
        );

        while !content.is_empty() {
            match self.check_boundary(content[0]) {
                // the current mail ils probably embedded.
                // we can stop parsing the mail and return it.
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "boundary found in regular message."
                    );
                    *content = &content[1..];
                    return Ok(body);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0],
                    )));
                }

                // we just skip the line & push the content in the body.
                None => body.push(content[0].to_string()),
            };
            *content = &content[1..];
        }

        // EOF reached.
        log::debug!(
            target: log_channels::PARSER,
            "EOF reached while storing body of regular message."
        );
        Ok(body)
    }

    // TODO: merge with @as_regular_body
    fn parse_regular_mime_body(&self, content: &mut &[&str]) -> ParserResult<Vec<String>> {
        let mut body = Vec::new();

        while !content.is_empty() {
            match self.check_boundary(content[0]) {
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    return Ok(body);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0],
                    )));
                }

                None => {
                    // we skip the header & body separation line.
                    if !(body.is_empty() && content[0].is_empty()) {
                        body.push(content[0].to_string());
                    }
                }
            };
            *content = &content[1..];
        }

        Ok(body)
    }

    fn as_mime_body(
        &mut self,
        content: &mut &[&str],
        headers: Vec<MimeHeader>,
        parent: Option<&[MimeHeader]>,
    ) -> ParserResult<Mime> {
        match get_mime_type(&headers, parent)? {
            ("message", sub_type) => {
                log::debug!(
                    target: log_channels::PARSER,
                    "'message' content type found (message/{})",
                    sub_type
                );
                *content = &content[1..];
                Ok(Mime {
                    headers,
                    content: MimeBodyType::Embedded(self.parse_inner(content)?),
                })
            }
            ("multipart", _) => {
                log::debug!(target: log_channels::PARSER, "parsing multipart.");
                Ok(Mime {
                    headers: headers.clone(),
                    content: MimeBodyType::Multipart(self.parse_multipart(&headers, content)?),
                })
            }
            (body_type, sub_type) => {
                log::debug!(
                    target: log_channels::PARSER,
                    "parsing regular mime section of type '{}' and subtype '{}'",
                    body_type,
                    sub_type
                );
                Ok(Mime {
                    headers,
                    content: MimeBodyType::Regular(self.parse_regular_mime_body(content)?),
                })
            }
        }
    }

    fn parse_mime(
        &mut self,
        content: &mut &[&str],
        parent: Option<&[MimeHeader]>,
    ) -> ParserResult<Mime> {
        let mut headers = Vec::new();

        log::debug!(target: log_channels::PARSER, "parsing a mime section.");

        while content.len() > 1 {
            if let Some((name, value)) = read_header(content) {
                log::debug!(
                    target: log_channels::PARSER,
                    "mime-header found: '{}' => '{}'.",
                    name,
                    value
                );
                headers.push(get_mime_header(&name, &value));
            } else {
                log::debug!(
                    target: log_channels::PARSER,
                    "finished reading mime headers, body found."
                );
                break;
            };
            *content = &content[1..];
        }

        self.as_mime_body(content, headers, parent)
    }

    fn parse_preamble<'a>(&self, content: &'a mut &[&str]) -> ParserResult<Vec<&'a str>> {
        log::debug!(
            target: log_channels::PARSER,
            "storing preamble for a multipart mime section."
        );
        let mut preamble = Vec::new();

        while content.len() > 1 {
            match self.check_boundary(content[0]) {
                Some(BoundaryType::Delimiter) => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "delimiter boundary found for multipart, finished storing preamble."
                    );
                    return Ok(preamble);
                }
                Some(BoundaryType::End) => {
                    return Err(ParserError::MisplacedBoundary(
                        "their should not be a end boundary in the preamble".to_string(),
                    ));
                }
                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0],
                    )));
                }
                None => preamble.push(content[0]),
            };

            *content = &content[1..];
        }

        Err(ParserError::BoundaryNotFound(
            "boundary not found after mime part preamble".to_string(),
        ))
    }

    fn parse_epilogue<'a>(&self, content: &'a mut &[&str]) -> ParserResult<Vec<&'a str>> {
        log::debug!(
            target: log_channels::PARSER,
            "storing epilogue for a multipart mime section."
        );
        let mut epilogue = Vec::new();

        while content.len() > 1 {
            match self.check_boundary(content[0]) {
                // there could be an ending or delimiting boundary,
                // meaning that the next lines will be part of another mime part.
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "boundary found for multipart, finished storing epilogue."
                    );
                    break;
                }
                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0],
                    )));
                }
                None => epilogue.push(content[0]),
            };
            *content = &content[1..];
        }

        Ok(epilogue)
    }

    fn parse_multipart(
        &mut self,
        headers: &[MimeHeader],
        content: &mut &[&str],
    ) -> ParserResult<MimeMultipart> {
        let content_type = headers.iter().find(|h| h.name == "content-type").unwrap();

        match content_type.args.get("boundary") {
            Some(b) => {
                log::debug!(
                    target: log_channels::PARSER,
                    "boundary found in parameters: '{}'.",
                    b
                );
                self.boundary_stack.push(b.to_string());
            }
            None => {
                return Err(ParserError::BoundaryNotFound(
                    "boundary parameter not found in Content-Type header for a multipart."
                        .to_string(),
                ))
            }
        };

        let mut multi_parts = MimeMultipart {
            preamble: self
                .parse_preamble(content)?
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\r\n"),
            parts: Vec::new(),
            epilogue: String::new(),
        };

        while content.len() > 1 {
            match self.check_boundary(content[0]) {
                Some(BoundaryType::Delimiter) => {
                    log::debug!(target: log_channels::PARSER,
                        "delimiter boundary found while parsing multipart: '{}', calling parse_mime.",
                        &content[0]
                    );
                    *content = &content[1..];

                    multi_parts
                        .parts
                        .push(self.parse_mime(content, Some(headers))?);
                }

                Some(BoundaryType::End) => {
                    log::debug!(target: log_channels::PARSER,
                        "end boundary found while parsing multipart: '{}', stopping multipart parsing.",
                        &content[0]
                    );
                    self.boundary_stack.pop();
                    *content = &content[1..];
                    multi_parts.epilogue = self
                        .parse_epilogue(content)?
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\r\n");
                    return Ok(multi_parts);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0],
                    )));
                }

                None => {
                    log::debug!(
                        target: log_channels::PARSER,
                        "EOF reached while parsing multipart.",
                    );
                    return Ok(multi_parts);
                }
            };
        }

        Ok(multi_parts)
    }
}

fn check_mandatory_headers(headers: &[(String, String)]) -> ParserResult<()> {
    /// rfc822 headers that requires to be specified.
    /// ? does they require ONLY to be at the root message ? (in case of embedded messages)
    const MANDATORY_HEADERS: [&str; 2] = ["from", "date"];

    for mh in MANDATORY_HEADERS {
        if !headers.iter().any(|h| h.0.as_str() == mh) {
            return Err(ParserError::MandatoryHeadersNotFound(mh.to_string()));
        }
    }

    Ok(())
}

/// take the name and value of a header and parses those to create
/// a MimeHeader struct.
///
/// # Arguments
///
/// * `name` - the name of the header.
/// * `value` - the value of the header (with all params, folded included if any).
///
/// # Return
///
/// * `Result<MimeHeader>` - a MimeHeader or a ParserError.
///
fn get_mime_header(name: &str, value: &str) -> MimeHeader {
    // cut the current line using the ";" separator into a vector of "arg=value" strings.
    let args = value.split(';').collect::<Vec<&str>>();
    let mut args_iter = args.iter();

    MimeHeader {
        name: name.to_string(),
        value: args_iter.next().unwrap().trim().to_lowercase(),

        // split every element of args by the "=" token (if there are any parameters).
        // inserts all resulting key / value pair into new_args.
        args: args_iter
            .into_iter()
            .filter_map(|arg| {
                let mut split = arg.splitn(2, '=');
                match (split.next(), split.next()) {
                    (Some(key), Some(value)) => Some((key, value)),
                    // no error here, bad arguments are just omitted.
                    _ => None,
                }
            })
            .map(|(key, value)| {
                (
                    key.trim().to_lowercase(),
                    match (value.find('"'), value.rfind('"')) {
                        (Some(first), Some(last)) if first < last => &value[first + 1..last],
                        _ => value,
                    }
                    // TODO: replace all characters specified in rfc.
                    .replace(&['\"', '\\'][..], ""),
                )
            })
            .collect::<std::collections::HashMap<String, String>>(),
    }
}

// check rfc2045 p.9. Additional MIME Header Fields.
#[inline]
fn is_mime_header(name: &str) -> bool {
    name.starts_with("content-")
}

// is used to deduce the boundary type.
// ! this method is called too many times, causing slow downs.
#[inline]
fn get_boundary_type(line: &str, boundary: &str) -> Option<BoundaryType> {
    match (
        // TODO: can be optimized.
        line.starts_with("--") && !line.starts_with(boundary),
        line.ends_with("--") && !line.ends_with(boundary),
        line.contains(boundary),
    ) {
        (true, false, true) => Some(BoundaryType::Delimiter),
        (true, true, true) => Some(BoundaryType::End),
        _ => None,
    }
}

/*
#[cfg(test)]
mod test {
    use super::*;

    // NOTE: things to consider:
    //       - header folding (does letter does it automatically ?)
    //       - comments (do we need to keep them ?)
    //       - boundaries
    //       -

    /// FIXME: a \n is added between the headers and the body
    #[test]
    #[ignore]
    fn test_to_raw() {
        let content = vec![
"x-mozilla-status: 0001",
"x-mozilla-status2: 01000000",
"x-mozilla-keys:                                                                                 ",
"fcc: imap://tabis%40localhost.com@localhost.com/sent",
"x-identity-key: id3",
"x-account-key: account4",
"from: tabis lucas <tabis@localhost>",
"subject: text content",
"to: tabis@localhost, green@example.com, foo@example.com, x@x.com",
"message-id: <51734671-2e09-946e-7e3f-ec59b83e82d0@localhost.com>",
"date: tue, 30 nov 2021 20:54:27 +0100",
"x-mozilla-draft-info: internal/draft; vcard=0; receipt=0; dsn=0; uuencode=0;",
" attachmentreminder=0; deliveryformat=1",
"user-agent: mozilla/5.0 (x11; linux x86_64; rv:78.0) gecko/20100101",
" thunderbird/78.14.0",
"mime-version: 1.0",
"content-type: text/plain; charset=utf-8; format=flowed",
"content-language: en-us",
"content-transfer-encoding: 7bit",
"",
"je ne suis qu'un contenu de texte."];

        let parsed = MailMimeParser::default()
            .parse(content.join("\n").as_bytes())
            .expect("parsing failed");

        assert_eq!(
            {
                let (headers, body) = parsed.to_raw();
                [headers, body].join("\n")
            },
            r#"x-mozilla-status: 0001
x-mozilla-status2: 01000000
x-mozilla-keys:
fcc: imap://tabis%40localhost.com@localhost.com/sent
x-identity-key: id3
x-account-key: account4
from: tabis lucas <tabis@localhost>
subject: text content
to: tabis@localhost, green@example.com, foo@example.com, x@x.com
message-id: <51734671-2e09-946e-7e3f-ec59b83e82d0@localhost.com>
date: tue, 30 nov 2021 20:54:27 +0100
x-mozilla-draft-info: internal/draft; vcard=0; receipt=0; dsn=0; uuencode=0; attachmentreminder=0; deliveryformat=1
user-agent: mozilla/5.0 gecko/20100101 thunderbird/78.14.0
mime-version: 1.0
content-type: text/plain; charset="utf-8"; format="flowed"
content-language: en-us
content-transfer-encoding: 7bit

je ne suis qu'un contenu de texte."#
        );
    }
}
*/
