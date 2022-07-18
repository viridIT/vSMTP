use crate::parser::MailMimeParser;
use vsmtp_common::{BodyType, Mail, MailHeaders, MailParser};

#[test]
fn simple() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            &include_str!("../../mail/rfc5322/A.2.a.eml")
                .lines()
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    ("from", "John Doe <jdoe@machine.example>"),
                    ("to", "Mary Smith <mary@example.net>"),
                    ("subject", "Saying Hello"),
                    ("date", "Fri, 21 Nov 1997 09:55:06 -0600"),
                    ("message-id", "<1234@local.machine.example>"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["This is a message just to say hello.", "So, \"Hello\"."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        include_str!("../../mail/rfc5322/A.2.a.eml").replace('\n', "\r\n")
    );
}

#[test]
fn reply_simple() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            &include_str!("../../mail/rfc5322/A.2.b.eml")
                .lines()
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    ("from", "Mary Smith <mary@example.net>"),
                    ("to", "John Doe <jdoe@machine.example>"),
                    (
                        "reply-to",
                        "\"Mary Smith: Personal Account\" <smith@home.example>"
                    ),
                    ("subject", "Re: Saying Hello"),
                    ("date", "Fri, 21 Nov 1997 10:01:10 -0600"),
                    ("message-id", "<3456@example.net>"),
                    ("in-reply-to", "<1234@local.machine.example>"),
                    ("references", "<1234@local.machine.example>"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["This is a reply to your hello."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        include_str!("../../mail/rfc5322/A.2.b.eml").replace('\n', "\r\n")
    );
}

#[test]
fn reply_reply() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            &include_str!("../../mail/rfc5322/A.2.c.eml")
                .lines()
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    (
                        "to",
                        "\"Mary Smith: Personal Account\" <smith@home.example>"
                    ),
                    ("from", "John Doe <jdoe@machine.example>"),
                    ("subject", "Re: Saying Hello"),
                    ("date", "Fri, 21 Nov 1997 11:00:00 -0600"),
                    ("message-id", "<abcd.1234@local.machine.test>"),
                    ("in-reply-to", "<3456@example.net>"),
                    (
                        "references",
                        "<1234@local.machine.example> <3456@example.net>"
                    ),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["This is a reply to your reply."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        include_str!("../../mail/rfc5322/A.2.c.eml").replace('\n', "\r\n")
    );
}
