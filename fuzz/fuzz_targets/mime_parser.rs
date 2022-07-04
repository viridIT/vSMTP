#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_common::MailParser;
use vsmtp_mail_parser::MailMimeParser;

fuzz_target!(|data: &[u8]| {
    let _ = std::str::from_utf8(data)
        .map(|data| MailMimeParser::default().parse_lines(&data.lines().collect::<Vec<_>>()));
});
