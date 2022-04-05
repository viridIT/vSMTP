use vsmtp_config::Config;

/// Get a config for local test
///
/// # Panics
///
/// * config cannot be built
#[must_use]
pub fn local_test() -> Config {
    Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name("testserver.com")
        .with_user_group_and_default_system("root", "root")
        .unwrap()
        .with_ipv4_localhost()
        .with_default_logs_settings()
        .with_spool_dir_and_default_queues("./tmp/delivery")
        .without_tls_support()
        .with_default_smtp_options()
        .with_default_smtp_error_handler()
        .with_default_smtp_codes()
        .without_auth()
        .with_default_app()
        .with_vsl("src/tests/empty_main.vsl")
        .with_default_app_logs()
        .without_services()
        .with_system_dns()
        .validate()
        .unwrap()
}
