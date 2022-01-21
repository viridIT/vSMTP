#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp::mime::parser::MailMimeParser;

fuzz_target!(|data: &[u8]| {
    let _ = MailMimeParser::default().parse(data);
});
