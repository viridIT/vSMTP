use crate::Config;

#[test]
fn parse() {
    let toml = include_str!("../../../../examples/config/simple.toml");
    pretty_assertions::assert_eq!(
        Config::from_toml(toml).unwrap(),
        Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server_name("my.fqdn.com")
            .with_user_group_and_default_system("root", "root")
            .unwrap()
            .with_interfaces(
                &["127.0.0.1:25".parse().unwrap()],
                &["127.0.0.1:587".parse().unwrap()],
                &["127.0.0.1:465".parse().unwrap()]
            )
            .with_default_logs_settings()
            .with_default_delivery()
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .without_services()
            .with_system_dns()
            .validate()
            .unwrap()
    );
}
