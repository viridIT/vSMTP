use crate::MailMimeParser;
use vsmtp_common::MailParser;

const MAIL: &str = include_str!("../mail/SimpleSubject.eml");

#[test]
fn mime_parser() {
    let parsed = MailMimeParser::default()
        .parse_lines(MAIL.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap();
    pretty_assertions::assert_eq!(MAIL, parsed.to_string());
}
