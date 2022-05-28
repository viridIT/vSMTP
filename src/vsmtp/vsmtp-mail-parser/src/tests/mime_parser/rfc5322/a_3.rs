use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

#[test]
fn resent() {
    assert_eq!(
        MailMimeParser::default()
            .parse(
                include_str!("../../mail/rfc5322/A.3.eml")
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
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
            .collect::<Vec<_>>(),
            body: BodyType::Regular(
                vec!["This is a message just to say hello.", "So, \"Hello\"."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }))
    );
}
