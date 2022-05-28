use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

const MAIL: &str = include_str!("../../mail/rfc5322/A.1.3.eml");

#[test]
fn group_addresses() {
    assert_eq!(
        MailMimeParser::default()
            .parse(MAIL.lines().map(str::to_string).collect::<Vec<_>>())
            .unwrap(),
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
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
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["Testing."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
}
