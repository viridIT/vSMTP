use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

const MAIL: &str = include_str!("../../mail/rfc5322/A.1.2.eml");

#[test]
fn types_mailboxes() {
    let parsed = MailMimeParser::default()
        .parse(MAIL.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
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
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["Hi everyone."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "from: \"Joe Q. Public\" <john.q.public@example.com>\r\n".to_string(),
            "to: Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>\r\n".to_string(),
            "cc: <boss@nil.test>, \"Giant; \\\"Big\\\" Box\" <sysservices@example.net>\r\n"
                .to_string(),
            "date: Tue, 1 Jul 2003 10:52:37 +0200\r\n".to_string(),
            "message-id: <5678.21-Nov-1997@example.com>\r\n".to_string(),
            "\r\n".to_string(),
            "Hi everyone.\r\n".to_string(),
        ]
        .concat()
    );
}
