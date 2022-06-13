use crate::parser::MailMimeParser;
use vsmtp_common::{
    mail_context::MessageBody,
    MailParser, {BodyType, Mail},
};

#[test]
fn white_space_and_comments() {
    let parsed = MailMimeParser::default()
        .parse_lines(
            include_str!("../../mail/rfc5322/A.5.eml")
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
                ("from", "Pete <pete@silly.test>"),
                (
                    "to",
                    concat!(
                        "A Group",
                        "     :Chris Jones <c@public.example>,",
                        "         joe@example.org,",
                        "  John <jdoe@one.test> ; ",
                    )
                ),
                ("cc", "Hidden recipients  :  ;"),
                (
                    "date",
                    concat!(
                        "Thu,",
                        "      13",
                        "        Feb",
                        "          1969",
                        "      23:32",
                        "               -0330 "
                    )
                ),
                ("message-id", "<testabcd.1234@silly.test>"),
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
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "from: Pete <pete@silly.test>\r\n".to_string(),
            "to: A Group     :Chris Jones <c@public.example>,         joe@example.org, \r\n"
                .to_string(),
            "\t John <jdoe@one.test> ; \r\n".to_string(),
            "cc: Hidden recipients  :  ;\r\n".to_string(),
            "date: Thu,      13        Feb          1969      23:32               -0330 \r\n"
                .to_string(),
            "message-id: <testabcd.1234@silly.test>\r\n".to_string(),
            "\r\n".to_string(),
            "Testing.\r\n".to_string(),
        ]
        .concat()
    );
}
