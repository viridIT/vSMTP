#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_config::Config;
use vsmtp_rule_engine::RuleEngine;

fuzz_target!(|data: &[u8]| {
    let config = Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_hostname()
        .with_default_system()
        .with_ipv4_localhost()
        .with_default_logs_settings()
        .with_spool_dir_and_default_queues("./tmp/fuzz")
        .without_tls_support()
        .with_default_smtp_options()
        .with_default_smtp_error_handler()
        .with_default_smtp_codes()
        .without_auth()
        .with_default_app()
        .with_vsl("./main.vsl")
        .with_default_app_logs()
        .with_system_dns()
        .without_virtual_entries()
        .validate()
        .unwrap();

    let _ = std::str::from_utf8(data).map(|script| RuleEngine::from_script(&config, script));
});
