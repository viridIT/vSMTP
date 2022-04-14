use vsmtp_common::mail_context::Body;

use crate::MailMimeParser;

fn generate_test_bodies() -> (Body, Body, Body) {
    let raw_email = r#"From: john <john@example.com>
To: green@example.com
Date: tue, 30 nov 2021 20:54:27 +0100
Content-Language: en-US
Subject: test message
Content-Type: text/html; charset=UTF-8
Content-Transfer-Encoding: 7bit
        
<html>
  <head>
    <meta http-equiv="content-type" content="text/html; charset=UTF-8">
  </head>
  <body>
    only plain text here<br>
  </body>
</html>
"#;

    (
        Body::Empty,
        Body::Raw(raw_email.to_string()),
        Body::Parsed(Box::new(
            MailMimeParser::default()
                .parse(raw_email.as_bytes())
                .unwrap(),
        )),
    )
}

#[test]
fn test_get_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (empty, raw, parsed) = generate_test_bodies();

    assert_eq!(empty.get_header("To"), None);
    assert_eq!(raw.get_header("To"), Some("green@example.com"));
    assert_eq!(parsed.get_header("to"), Some("green@example.com"));
}

#[test]
fn test_set_and_add_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (mut empty, mut raw, mut parsed) = generate_test_bodies();

    let new_header = "X-New-Header";
    let new_header_message = "this is a new header!";
    let subject_message = "this is a subject";

    empty.set_header("To", "john@example.com");
    empty.set_header(new_header, new_header_message);
    assert_eq!(empty.get_header("To"), None);
    assert_eq!(empty.get_header(new_header), None);

    raw.set_header("Subject", subject_message);
    raw.set_header(new_header, new_header_message);
    assert_eq!(raw.get_header("Subject"), Some(subject_message));
    assert_eq!(raw.get_header(new_header), Some(new_header_message));

    parsed.set_header("subject", subject_message);
    parsed.set_header(new_header, new_header_message);
    assert_eq!(parsed.get_header("subject"), Some(subject_message));
    assert_eq!(parsed.get_header(new_header), Some(new_header_message));
}
