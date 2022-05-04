#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_config::Config;

fuzz_target!(|data: &[u8]| {
    let _ = std::str::from_utf8(data).map(Config::from_toml);
});
