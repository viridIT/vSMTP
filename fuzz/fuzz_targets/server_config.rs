#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp::config::server_config::ServerConfig;

fuzz_target!(|data: &[u8]| {
    match toml::from_slice::<ServerConfig>(data) {
        Ok(config) => println!("{:?}", config),
        Err(_) => return,
    };
});
