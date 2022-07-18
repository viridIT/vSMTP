use crate::parser::MailMimeParser;
use vsmtp_common::{BodyType, Mail, MailHeaders, MailParser};

const MAIL: &str = include_str!("../../mail/rfc5322/A.1.3.eml");

#[test]
fn group_addresses() {
    let parsed = MailMimeParser::default()
        .parse_lines(&MAIL.lines().collect::<Vec<_>>())
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    ("from", "Pete <pete@silly.example>"),
                    (
                        "to",
                        "A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;"
                    ),
                    ("cc", "Undisclosed recipients:;"),
                    ("date", "Thu, 13 Feb 1969 23:32:54 -0330"),
                    ("message-id", "<testabcd.1234@silly.example>"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["Testing."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }
    );
    pretty_assertions::assert_eq!(parsed.to_string(), MAIL.replace('\n', "\r\n"));
}
