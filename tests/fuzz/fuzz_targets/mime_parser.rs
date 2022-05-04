#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_mail_parser::MailMimeParser;

fuzz_target!(|data: &[u8]| {
    let _ = vsmtp_common::MailParser::parse(&mut MailMimeParser::default(), data);
});
