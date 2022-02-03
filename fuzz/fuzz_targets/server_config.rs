#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp::config::server_config::ServerConfig;

fuzz_target!(|data: &[u8]| {
    let _ = std::str::from_utf8(data).map(ServerConfig::from_toml);
});
