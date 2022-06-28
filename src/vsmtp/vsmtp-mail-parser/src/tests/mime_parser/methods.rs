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
use vsmtp_common::mail_context::MessageBody;

use crate::MailMimeParser;

fn generate_test_bodies() -> (MessageBody, MessageBody) {
    let headers = [
        "From: john <john@example.com>",
        "To: green@example.com",
        "Date: tue, 30 nov 2021 20:54:27 +0100",
        "Content-Language: en-US",
        "Subject: test message",
        "Content-Type: text/html; charset=UTF-8",
        "Content-Transfer-Encoding: 7bit",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    let body = r#"<html>
  <head>
    <meta http-equiv="content-type" content="text/html; charset=UTF-8">
  </head>
  <body>
    only plain text here<br>
  </body>
</html>
"#
    .to_string();

    let raw = MessageBody::Raw { headers, body };
    let mut parsed = raw.clone();
    parsed.to_parsed::<MailMimeParser>().unwrap();

    (raw, parsed)
}

#[test]
fn test_get_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (raw, parsed) = generate_test_bodies();

    assert_eq!(raw.get_header("To"), Some("green@example.com"));
    assert_eq!(parsed.get_header("to"), Some("green@example.com"));
}

#[test]
fn test_set_and_append_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (mut raw, mut parsed) = generate_test_bodies();

    let new_header = "X-New-Header";
    let new_header_message = "this is a new header!";
    let subject_message = "this is a subject";

    raw.set_header("Subject", subject_message);
    raw.set_header(new_header, new_header_message);
    assert_eq!(raw.get_header("Subject"), Some(subject_message));
    assert_eq!(raw.get_header(new_header), Some(new_header_message));

    parsed.set_header("subject", subject_message);
    parsed.set_header(new_header, new_header_message);
    assert_eq!(parsed.get_header("subject"), Some(subject_message));
    assert_eq!(parsed.get_header(new_header), Some(new_header_message));
}
