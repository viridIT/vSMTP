use vsmtp_config::{Config, ConfigServerSMTPAuth};

pub fn safe_auth_config() -> Config {
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
        .with_safe_auth(false, -1)
        .with_default_app()
        .with_vsl("./src/tests/empty_main.vsl")
        .with_default_app_logs()
        .without_services()
        .with_system_dns()
        .without_virtual_entries()
        .validate()
        .unwrap()
}

pub fn unsafe_auth_config() -> Config {
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
        .with_auth(false, true, ConfigServerSMTPAuth::default_mechanisms(), -1)
        .with_default_app()
        .with_vsl("./src/tests/auth.vsl")
        .with_default_app_logs()
        .without_services()
        .with_system_dns()
        .without_virtual_entries()
        .validate()
        .unwrap()
}

mod all_mechanism;
mod basic;
