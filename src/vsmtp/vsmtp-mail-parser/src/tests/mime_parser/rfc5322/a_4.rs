use crate::parser::MailMimeParser;
use vsmtp_common::{BodyType, Mail, MailHeaders, MailParser};

#[test]
fn tracing() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            &include_str!("../../mail/rfc5322/A.4.eml")
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
                        "received",
                        concat!(
                            "from x.y.test\r\n",
                            "  by example.net\r\n",
                            "  via TCP\r\n",
                            "  with ESMTP\r\n",
                            "  id ABC12345\r\n",
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
        include_str!("../../mail/rfc5322/A.4.eml").replace('\n', "\r\n")
    );
}
