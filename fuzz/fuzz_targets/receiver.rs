#![no_main]
use libfuzzer_sys::fuzz_target;
use vsmtp_common::{code::SMTPReplyCode, mail_context::MailContext, re::anyhow};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::receiver::{
    handle_connection, test_helpers::Mock, Connection, ConnectionKind, IoService, OnMail,
};

struct FuzzOnMail;

#[async_trait::async_trait]
impl OnMail for FuzzOnMail {
    async fn on_mail<S: std::io::Read + std::io::Write + Send>(
        &mut self,
        conn: &mut Connection<'_, S>,
        _: Box<MailContext>,
        _: &mut Option<String>,
    ) -> anyhow::Result<()> {
        conn.send_code(SMTPReplyCode::Code250)?;
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let mut config = Config::builder()
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
        .without_services()
        .with_system_dns()
        .validate()
        .unwrap();
    config.server.smtp.error.soft_count = -1;
    config.server.smtp.error.hard_count = -1;

    let config = std::sync::Arc::new(config);

    let mut written_data = Vec::new();
    let mut mock = Mock::new(std::io::Cursor::new(data.to_vec()), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::new(
        ConnectionKind::Opportunistic,
        "0.0.0.0:0".parse().unwrap(),
        config,
        &mut io,
    );

    let re = std::sync::Arc::new(std::sync::RwLock::new(
        RuleEngine::new(&None).expect("failed to build rule engine"),
    ));

    let _ = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(handle_connection(
            &mut conn,
            None,
            None,
            re,
            &mut FuzzOnMail,
        ));
});
