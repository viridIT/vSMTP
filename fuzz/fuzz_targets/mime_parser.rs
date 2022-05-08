#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_common::MailParser;
use vsmtp_mail_parser::MailMimeParser;

fuzz_target!(|data: &[u8]| {
    let _ = MailMimeParser::default().parse(data);
});
