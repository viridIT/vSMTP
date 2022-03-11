use vsmtp_common::mail::{BodyType, Mail};

use crate::parser::MailMimeParser;

#[test]
fn white_space_and_comments() {
    assert_eq!(
        MailMimeParser::default()
            .parse(include_bytes!("../../mail/rfc5322/A.5.eml"))
            .unwrap(),
        Mail {
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
        }
    );
}
