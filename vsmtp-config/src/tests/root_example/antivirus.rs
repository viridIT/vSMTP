use vsmtp_common::collection;

use crate::{Config, Service};

#[test]
fn parse() {
    let toml = include_str!("../../../../examples/config/antivirus.toml");
    pretty_assertions::assert_eq!(
        Config::from_toml(toml).unwrap(),
        Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_hostname()
            .with_default_system()
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_default_delivery()
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .without_auth()
            .with_default_app()
            .with_vsl("../examples/config/antivirus/main.vsl")
            .with_default_app_logs()
            .with_services(collection! {
                "clamscan".to_string() => Service::UnixShell {
                    timeout: std::time::Duration::from_secs(15),
                    user: None,
                    group: None,
                    command: "../examples/config/antivirus/clamscan.sh".to_string(),
                    args: Some("{mail}".to_string())
                }
            })
            .with_system_dns()
            .without_virtual_entries()
            .validate()
            .unwrap()
    );
}
