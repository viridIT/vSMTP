#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig, model::mail::MailContext, resolver::DataEndResolver,
        rules::tests::helpers::run_integration_engine_test, smtp::code::SMTPReplyCode,
    };

    struct Test;

    #[async_trait::async_trait]
    impl DataEndResolver for Test {
        async fn on_data_end(
            _: &ServerConfig,
            _: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_connect_rules() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/connect/valid_connect.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"220 test.server.com Service ready\r\n"
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/connect/invalid_connect.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"",
        )
        .await
        .is_err());
    }
}
