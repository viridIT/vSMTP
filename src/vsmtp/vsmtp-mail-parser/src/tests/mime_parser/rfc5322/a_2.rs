use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

#[test]
fn simple() {
    let parsed = MailMimeParser::default()
        .parse(
            include_str!("../../mail/rfc5322/A.2.a.eml")
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
                ("from", "John Doe <jdoe@machine.example>"),
                ("to", "Mary Smith <mary@example.net>"),
                ("subject", "Saying Hello"),
                ("date", "Fri, 21 Nov 1997 09:55:06 -0600"),
                ("message-id", "<1234@local.machine.example>"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["This is a message just to say hello.", "So, \"Hello\"."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "from: John Doe <jdoe@machine.example>\r\n".to_string(),
            "to: Mary Smith <mary@example.net>\r\n".to_string(),
            "subject: Saying Hello\r\n".to_string(),
            "date: Fri, 21 Nov 1997 09:55:06 -0600\r\n".to_string(),
            "message-id: <1234@local.machine.example>\r\n".to_string(),
            "\r\n".to_string(),
            "This is a message just to say hello.\r\n".to_string(),
            "So, \"Hello\".\r\n".to_string(),
        ]
        .concat()
    );
}

#[test]
fn reply_simple() {
    let parsed = MailMimeParser::default()
        .parse(
            include_str!("../../mail/rfc5322/A.2.b.eml")
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
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
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["This is a reply to your hello."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "from: Mary Smith <mary@example.net>\r\n".to_string(),
            "to: John Doe <jdoe@machine.example>\r\n".to_string(),
            "reply-to: \"Mary Smith: Personal Account\" <smith@home.example>\r\n".to_string(),
            "subject: Re: Saying Hello\r\n".to_string(),
            "date: Fri, 21 Nov 1997 10:01:10 -0600\r\n".to_string(),
            "message-id: <3456@example.net>\r\n".to_string(),
            "in-reply-to: <1234@local.machine.example>\r\n".to_string(),
            "references: <1234@local.machine.example>\r\n".to_string(),
            "\r\n".to_string(),
            "This is a reply to your hello.\r\n".to_string(),
        ]
        .concat()
    );
}

#[test]
fn reply_reply() {
    let parsed = MailMimeParser::default()
        .parse(
            include_str!("../../mail/rfc5322/A.2.c.eml")
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
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
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["This is a reply to your reply."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "to: \"Mary Smith: Personal Account\" <smith@home.example>\r\n".to_string(),
            "from: John Doe <jdoe@machine.example>\r\n".to_string(),
            "subject: Re: Saying Hello\r\n".to_string(),
            "date: Fri, 21 Nov 1997 11:00:00 -0600\r\n".to_string(),
            "message-id: <abcd.1234@local.machine.test>\r\n".to_string(),
            "in-reply-to: <3456@example.net>\r\n".to_string(),
            "references: <1234@local.machine.example> <3456@example.net>\r\n".to_string(),
            "\r\n".to_string(),
            "This is a reply to your reply.\r\n".to_string(),
        ]
        .concat()
    );
}
