use crate::parser::MailMimeParser;
use vsmtp_common::{
    MailHeaders, MailParser, MessageBody, {BodyType, Mail},
};

const MAIL: &str = include_str!("../../mail/rfc5322/A.1.2.eml");

#[test]
fn types_mailboxes() {
    let parsed = MailMimeParser::default()
        .parse_lines(MAIL.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: MailHeaders(
                [
                    ("from", "\"Joe Q. Public\" <john.q.public@example.com>"),
                    (
                        "to",
                        "Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>"
                    ),
                    (
                        "cc",
                        "<boss@nil.test>, \"Giant; \\\"Big\\\" Box\" <sysservices@example.net>"
                    ),
                    ("date", "Tue, 1 Jul 2003 10:52:37 +0200"),
                    ("message-id", "<5678.21-Nov-1997@example.com>"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["Hi everyone."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
    pretty_assertions::assert_eq!(parsed.to_string(), MAIL.replace('\n', "\r\n"));
}
