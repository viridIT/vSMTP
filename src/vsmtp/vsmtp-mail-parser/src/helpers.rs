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
use crate::error::{ParserError, ParserResult};
use vsmtp_common::{re::anyhow, MimeHeader};

#[inline]
pub(super) fn has_wsc(input: &str) -> bool {
    input.starts_with(|c| c == ' ' || c == '\t')
}

/// See https://datatracker.ietf.org/doc/html/rfc5322#page-11
pub(super) fn remove_comments(line: &str) -> anyhow::Result<String> {
    let (depth, is_escaped, output) = line.chars().into_iter().fold(
        (0, false, String::with_capacity(line.len())),
        |(depth, is_escaped, mut output), elem| {
            if !is_escaped {
                if elem == '(' {
                    return (depth + 1, false, output);
                } else if elem == ')' {
                    return (depth - i32::from(depth > 0), false, output);
                }
            }

            if depth == 0 {
                output.push(elem);
            }

            (depth, elem == '\\', output)
        },
    );

    if depth != 0 || is_escaped {
        anyhow::bail!("something went wrong")
    }
    Ok(output)
}

/// cut the mime type of the current section and return the type and subtype.
/// if no content-type header is found, will check the parent for a default
/// content-type header value.
///
/// see https://datatracker.ietf.org/doc/html/rfc2045#page-14 for default content-type.
/// see https://datatracker.ietf.org/doc/html/rfc2046#page-26 for digest multipart parent.
pub(super) fn get_mime_type<'a>(
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

/// read the current line or folded content and extracts a header if there is any.
///
/// # Arguments
///
/// * `content` - the buffer of lines to parse. this function has the right
///               to iterate through the buffer because it can parse folded
///               headers.
///
/// # Return
///
/// * `Option<(String, String)>` - an option containing two strings,
///                                the name and value of the header parsed
pub(super) fn read_header(content: &mut &[&str]) -> Option<(String, String)> {
    let mut split = content[0].splitn(2, ':');

    match (split.next(), split.next()) {
        (Some(header), Some(field)) => Some((
            header.trim().to_ascii_lowercase(),
            remove_comments(
                // NOTE: was previously String + String, check for performance.
                &(format!(
                    "{}{}",
                    field.trim(),
                    content[1..]
                        .iter()
                        .take_while(|s| has_wsc(s))
                        .map(|s| {
                            *content = &content[1..];
                            &s[..]
                        })
                        .collect::<Vec<&str>>()
                        .join("")
                )),
            )
            .unwrap(),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_header() {
        let input = vec![
            "User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101",
            " Thunderbird/78.8.1",
        ];
        assert_eq!(
            read_header(&mut (&input[..])),
            Some((
                "user-agent".to_string(),
                "Mozilla/5.0  Gecko/20100101 Thunderbird/78.8.1".to_string()
            ))
        );
    }
}
