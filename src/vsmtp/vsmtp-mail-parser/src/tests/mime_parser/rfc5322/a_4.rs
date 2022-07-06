use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

#[test]
fn tracing() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            include_str!("../../mail/rfc5322/A.4.eml")
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
                    "received",
                    concat!(
                        "from x.y.test",
                        "  by example.net",
                        "  via TCP",
                        "  with ESMTP",
                        "  id ABC12345",
                        "  for <mary@example.net>;  21 Nov 1997 10:05:43 -0600",
                    )
                ),
                (
                    "received",
                    "from node.example by x.y.test; 21 Nov 1997 10:01:22 -0600"
                ),
                ("from", "John Doe <jdoe@node.example>"),
                ("to", "Mary Smith <mary@example.net>"),
                ("subject", "Saying Hello"),
                ("date", "Fri, 21 Nov 1997 09:55:06 -0600"),
                ("message-id", "<1234@local.node.example>"),
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
            "received: from x.y.test  by example.net  via TCP  with ESMTP  id ABC12345 "
                .to_string(),
            " for <mary@example.net>;  21 Nov 1997 10:05:43 -0600\r\n".to_string(),
            "received: from node.example by x.y.test; 21 Nov 1997 10:01:22 -0600\r\n".to_string(),
            "from: John Doe <jdoe@node.example>\r\n".to_string(),
            "to: Mary Smith <mary@example.net>\r\n".to_string(),
            "subject: Saying Hello\r\n".to_string(),
            "date: Fri, 21 Nov 1997 09:55:06 -0600\r\n".to_string(),
            "message-id: <1234@local.node.example>\r\n".to_string(),
            "\r\n".to_string(),
            "This is a message just to say hello.\r\n".to_string(),
            "So, \"Hello\".\r\n".to_string(),
        ]
        .concat()
    );
}
