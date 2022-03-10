use vsmtp_common::mail::{BodyType, Mail};

use crate::parser::MailMimeParser;

#[test]
fn simple() {
    assert_eq!(
        MailMimeParser::default()
            .parse(include_bytes!("../../mail/rfc5322/A.1.1.a.eml"))
            .unwrap(),
        Mail {
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
        }
    );
}

#[test]
fn forward() {
    assert_eq!(
        MailMimeParser::default()
            .parse(include_bytes!("../../mail/rfc5322/A.1.1.b.eml"))
            .unwrap(),
        Mail {
            headers: vec![
                ("from", "John Doe <jdoe@machine.example>"),
                ("sender", "Michael Jones <mjones@machine.example>"),
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
        }
    );
}
