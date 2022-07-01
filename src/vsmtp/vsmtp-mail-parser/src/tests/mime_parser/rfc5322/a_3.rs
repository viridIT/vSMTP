use crate::parser::MailMimeParser;
use vsmtp_common::{BodyType, Mail, MailHeaders, MailParser};

#[test]
fn resent() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            &include_str!("../../mail/rfc5322/A.3.eml")
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
                    ("resent-from", "Mary Smith <mary@example.net>"),
                    ("resent-to", "Jane Brown <j-brown@other.example>"),
                    ("resent-date", "Mon, 24 Nov 1997 14:22:01 -0800"),
                    ("resent-message-id", "<78910@example.net>"),
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
        include_str!("../../mail/rfc5322/A.3.eml").replace('\n', "\r\n")
    );
}
